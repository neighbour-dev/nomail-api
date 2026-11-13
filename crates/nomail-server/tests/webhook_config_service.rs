use actix_web::{App, http::StatusCode, test, web};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use hmac::{Hmac, Mac};
use nomail_server::{
    AppState, app,
    config::Config,
    db,
    entity::{user, webhook},
    realtime::Hub,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};
use sha2::Sha256;
mod common;

#[actix_web::test]
async fn config_service_preserves_sparse_concurrent_updates_credentials_and_scope() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: dir.path().join("configs.db").to_str().unwrap().into(),
        jwt_secret: "config-service-test-secret".into(),
        jwt_issuer: "test".into(),
        encryption_key: [7; 32],
        port: 3000,
    };
    let database = db::connect(&config).await.unwrap();
    common::initialize(&database, &config).await.unwrap();
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
    });
    let app = test::init_service(App::new().app_data(state).configure(app::configure)).await;
    let response = test::call_service(&app, test::TestRequest::post().uri("/api/auth/login").set_json(json!({"username":"admin","password":"password123","deviceId":"a","platform":"ios","deviceType":"phone"})).to_request()).await;
    let login: Value = test::read_body_json(response).await;
    let token = login["data"]["accessToken"].as_str().unwrap();
    macro_rules! api {
        ($method:ident, $path:expr, $body:expr) => {{
            let response = test::call_service(
                &app,
                test::TestRequest::$method()
                    .uri($path)
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .set_json($body)
                    .to_request(),
            )
            .await;
            let status = response.status();
            let body: Value = test::read_body_json(response).await;
            (status, body["data"].clone())
        }};
    }
    let (status, created) = api!(
        post,
        "/api/webhook/configs",
        json!({"name":"Original","platform":{"provider":"generic","credentials":{"type":"hmacSha256","secret":"original-secret"}},"description":"original-description"})
    );
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["enabled"], true);
    assert_eq!(created["key"].as_str().unwrap().len(), 32);
    assert!(created.get("secret").is_none() && created.get("encryptedSecret").is_none());
    let id = created["id"].as_i64().unwrap();
    let path = format!("/api/webhook/configs/{id}");
    let initial = webhook::Entity::find_by_id(id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(
            &nomail_app::security::decrypt(&initial.encrypted_platform, &[7; 32]).unwrap()
        )
        .unwrap()["credentials"]["secret"],
        "original-secret"
    );
    assert_eq!(api!(patch, &path, json!({})).1, created);
    for field in [
        "name",
        "platform",
        "description",
        "provider",
        "source",
        "secret",
    ] {
        let mut invalid_update = json!({});
        invalid_update[field] = Value::Null;
        assert_eq!(
            api!(patch, &path, invalid_update).0,
            StatusCode::BAD_REQUEST
        );
        let mut invalid_create = json!({"name":"Invalid", "platform":{"provider":"generic","credentials":{"type":"none"}}});
        invalid_create[field] = Value::Null;
        assert_eq!(
            api!(post, "/api/webhook/configs", invalid_create).0,
            StatusCode::BAD_REQUEST
        );
    }
    for invalid in [
        json!({}),
        json!([]),
        json!(["Name", "generic"]),
        json!({"name":"Missing"}),
        json!({"provider":"generic"}),
        json!({"name":"", "provider":"generic"}),
        json!({"name":"Name", "provider":""}),
    ] {
        assert_eq!(
            api!(post, "/api/webhook/configs", invalid).0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        webhook::Entity::find().all(&database).await.unwrap().len(),
        1
    );
    assert_eq!(
        webhook::Entity::find_by_id(id)
            .one(&database)
            .await
            .unwrap()
            .unwrap(),
        initial
    );
    for i in 0..6 {
        let rename = test::TestRequest::patch()
            .uri(&path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(json!({"name":format!("Name {i}")}))
            .to_request();
        let description = test::TestRequest::patch()
            .uri(&path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(json!({"description":format!("Description {i}")}))
            .to_request();
        let (rename, description) = tokio::join!(
            test::call_service(&app, rename),
            test::call_service(&app, description)
        );
        assert_eq!(rename.status(), StatusCode::OK);
        assert_eq!(description.status(), StatusCode::OK);
        let (_, current) = api!(get, &path, json!({}));
        assert_eq!(current["name"], format!("Name {i}"));
        assert_eq!(current["description"], format!("Description {i}"));
        assert!(current.get("source").is_none());
        assert_eq!(current["key"], created["key"]);
        let row = webhook::Entity::find_by_id(id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.encrypted_platform, initial.encrypted_platform);
        assert_eq!(row.created_at, initial.created_at);
    }
    let before = webhook::Entity::find_by_id(id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    api!(patch, &path, json!({}));
    assert_eq!(
        webhook::Entity::find_by_id(id)
            .one(&database)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    assert_eq!(
        api!(patch, &path, json!({"provider":"unsupported"})).0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        webhook::Entity::find_by_id(id)
            .one(&database)
            .await
            .unwrap()
            .unwrap(),
        before
    );
    api!(
        patch,
        &path,
        json!({"platform":{"provider":"generic","credentials":{"type":"hmacSha256","secret":"replacement-secret"}}})
    );
    let replaced = webhook::Entity::find_by_id(id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(replaced.encrypted_platform, initial.encrypted_platform);
    assert_eq!(
        serde_json::from_str::<Value>(
            &nomail_app::security::decrypt(&replaced.encrypted_platform, &[7; 32]).unwrap()
        )
        .unwrap()["credentials"]["secret"],
        "replacement-secret"
    );
    assert_eq!(replaced.key, initial.key);
    let enabled_path = format!("{path}/enabled");
    assert_eq!(
        api!(patch, &enabled_path, json!({"enabled":false})).1["enabled"],
        false
    );
    assert_eq!(
        api!(patch, &enabled_path, json!({"enabled":true})).1["enabled"],
        true
    );
    let now = Utc::now();
    let other = user::ActiveModel {
        username: Set("other-config-owner".into()),
        password_hash: Set("unused".into()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    let foreign = webhook::ActiveModel {
        user_id: Set(other.id),
        name: Set("Foreign".into()),
        key: Set("foreign-config-key".into()),
        provider: Set("generic".into()),
        encrypted_platform: Set(initial.encrypted_platform.clone()),
        enabled: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    let foreign_path = format!("/api/webhook/configs/{}", foreign.id);
    assert_eq!(api!(get, &foreign_path, json!({})).0, StatusCode::NOT_FOUND);
    assert_eq!(
        api!(patch, &foreign_path, json!({"name":"stolen"})).0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api!(
            patch,
            &format!("{foreign_path}/enabled"),
            json!({"enabled":false})
        )
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api!(delete, &foreign_path, json!({})).0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        webhook::Entity::find_by_id(foreign.id)
            .one(&database)
            .await
            .unwrap()
            .unwrap(),
        foreign
    );
    assert_eq!(api!(get, "/api/webhook/configs", json!({})).1["total"], 1);
    assert_eq!(
        api!(
            get,
            "/api/webhook/configs?page=9223372036854775807&pageSize=100",
            json!({})
        )
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        api!(
            get,
            "/api/webhook/configs?page=9007199254740994&pageSize=1",
            json!({})
        )
        .1["items"],
        json!([])
    );
    assert_eq!(api!(delete, &path, json!({})).0, StatusCode::OK);
    let deleted = webhook::Entity::find_by_id(id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deleted.deleted_at, Some(deleted.updated_at));
    assert_eq!(
        api!(patch, &path, json!({"name":"revived"})).0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api!(patch, &enabled_path, json!({"enabled":true})).0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(api!(delete, &path, json!({})).0, StatusCode::NOT_FOUND);
    assert_eq!(api!(get, &path, json!({})).0, StatusCode::NOT_FOUND);
    assert_eq!(
        webhook::Entity::find_by_id(id)
            .one(&database)
            .await
            .unwrap()
            .unwrap(),
        deleted
    );
    assert_eq!(api!(get, "/api/webhook/configs", json!({})).1["total"], 0);
    // Every exposed provider survives encrypted persistence and feeds the actual
    // receiving path. These are synthetic payloads and credentials only.
    for platform in [
        json!({"provider":"generic","credentials":{"type":"hmacSha256","secret":"platform-secret"}}),
        json!({"provider":"github","credentials":{"secret":"platform-secret"}}),
        json!({"provider":"gitea","credentials":{"secret":"platform-secret"}}),
        json!({"provider":"appstoreconnect","credentials":{"secret":"platform-secret"}}),
        json!({"provider":"gitlab","credentials":{"type":"signingToken","signingToken":"whsec_cGxhdGZvcm0tc2VjcmV0"}}),
        json!({"provider":"gitlab","credentials":{"type":"secretToken","secretToken":"platform-secret"}}),
        json!({"provider":"stripe","credentials":{"signingSecrets":["whsec_retired","whsec_platform-secret"]}}),
        json!({"provider":"slack","credentials":{"signingSecret":"platform-secret"}}),
        json!({"provider":"feishu","credentials":{"verificationToken":"platform-secret"}}),
        json!({"provider":"telegram","credentials":{"secretToken":"platform-secret"}}),
        json!({"provider":"shopify","credentials":{"clientSecret":"platform-secret"}}),
    ] {
        let provider = platform["provider"].as_str().unwrap();
        let (status, created) = api!(
            post,
            "/api/webhook/configs",
            json!({"name":"Platform test","platform":platform})
        );
        assert_eq!(status, StatusCode::OK, "{provider}: {created}");
        assert_eq!(created["platform"]["provider"], provider);
        assert!(created["platform"].get("credentials").is_none());
        assert!(!created.to_string().contains("platform-secret"));
        let config_path = format!("/api/webhook/configs/{}", created["id"]);
        let receive_path = created["url"].as_str().unwrap();
        assert_eq!(api!(get, &config_path, json!({})).1, created);
        let (raw, headers) = platform_delivery(provider, &platform);
        let mut request = test::TestRequest::post()
            .uri(receive_path)
            .insert_header(("Content-Type", "application/json"));
        for (name, value) in &headers {
            request = request.append_header((name.as_str(), value.as_str()));
        }
        request = request
            .append_header((
                "X-NoMail-Title",
                actix_web::http::header::HeaderValue::from_bytes("中文".as_bytes()).unwrap(),
            ))
            .append_header((
                "X-Raw-Binary",
                actix_web::http::header::HeaderValue::from_bytes(&[0xff]).unwrap(),
            ));
        let response =
            test::call_service(&app, request.set_payload(raw.clone()).to_request()).await;
        assert_eq!(response.status(), StatusCode::OK, "{provider}");
        let received: Value = test::read_body_json(response).await;
        assert!(
            received["data"]["messageId"].is_string(),
            "{provider}: {received}"
        );
        let message_path = format!(
            "/api/messages/{}",
            received["data"]["messageId"].as_str().unwrap()
        );
        let detail = api!(get, &message_path, json!({})).1;
        assert_eq!(detail["provider"], provider);
        let envelope = &detail["platformData"]["event"];
        assert_eq!(envelope.as_object().unwrap().len(), 2);
        assert!(envelope["header"].is_object());
        assert!(envelope.get("payload").is_some());
        assert!(!envelope.to_string().contains("platform-secret"));
        let mut expected_payload: Value = serde_json::from_str(&raw).unwrap();
        if provider == "appstoreconnect" {
            expected_payload.as_object_mut().unwrap().remove("future");
        }
        if provider == "feishu" {
            expected_payload["header"]
                .as_object_mut()
                .unwrap()
                .remove("token");
        }
        assert_eq!(
            envelope["payload"], expected_payload,
            "{provider}: persistence changed presence/value or added defaults"
        );
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("{message_path}/raw"))
                .insert_header(("Authorization", format!("Bearer {token}")))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("Content-Type").unwrap(),
            "application/json"
        );
        let original: Value = test::read_body_json(response).await;
        assert_eq!(original["code"], 0);
        assert_eq!(original["msg"], "success");
        let original = &original["data"];
        assert_eq!(original.as_object().unwrap().len(), 3);
        assert_eq!(original["contentType"], "application/json");
        assert_eq!(
            serde_json::from_str::<Value>(original["payload"].as_str().unwrap()).unwrap(),
            serde_json::from_str::<Value>(&raw).unwrap()
        );
        let stored = nomail_server::entity::message::Entity::find_by_id(
            uuid::Uuid::parse_str(received["data"]["messageId"].as_str().unwrap()).unwrap(),
        )
        .one(&database)
        .await
        .unwrap()
        .unwrap();
        assert_eq!(original["payload"], stored.raw_payload);
        assert_eq!(original["contentType"], stored.content_type);
        assert_eq!(
            original["header"],
            serde_json::from_str::<Value>(&stored.raw_headers).unwrap()
        );
        let raw_headers = original["header"].as_array().unwrap();
        assert!(raw_headers.iter().any(|h| h["name"] == "x-raw-binary"
            && h["value"] == "/w=="
            && h["encoding"] == "base64"));
        assert!(
            raw_headers
                .iter()
                .any(|h| h["name"] == "x-nomail-title" && h["value"] == "中文")
        );
        assert!(envelope["header"].get("title").is_none());
        assert!(
            raw_headers
                .iter()
                .all(|h| !nomail_app::webhook::is_raw_header_blocked(h["name"].as_str().unwrap()))
        );
        assert!(
            !original["header"]
                .to_string()
                .contains("raw-private-credential")
        );
        assert!(
            raw_headers
                .iter()
                .any(|h| h["name"] == "x-raw-unknown" && h["value"] == "retained")
        );
        assert!(
            raw_headers
                .iter()
                .any(|h| h["name"] == "x-raw-empty" && h["value"] == "")
        );
        assert_eq!(
            raw_headers
                .iter()
                .filter(|h| h["name"] == "x-raw-repeated")
                .map(|h| h["value"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        if provider == "github" {
            let fixtures: Value = serde_json::from_str(include_str!(
                "../../nomail-core/src/webhook/platforms/github/fixtures/deliveries.json"
            ))
            .unwrap();
            for fixture in fixtures.as_array().unwrap() {
                let body = fixture["payload"].to_string();
                // Encoding every byte is valid form encoding and keeps the
                // exact signed wire bytes independent of test-only libraries.
                let mut form = String::from("payload=");
                for byte in body.bytes() {
                    use std::fmt::Write;
                    write!(&mut form, "%{byte:02X}").unwrap();
                }
                let mut message_id = Value::Null;
                for (wire, content_type, duplicate) in [
                    (&body, "application/json", false),
                    (&form, "application/x-www-form-urlencoded", true),
                ] {
                    let mut mac = Hmac::<Sha256>::new_from_slice(b"platform-secret").unwrap();
                    mac.update(wire.as_bytes());
                    let response = test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri(receive_path)
                            .insert_header(("Content-Type", content_type))
                            .insert_header(("X-GitHub-Event", fixture["event"].as_str().unwrap()))
                            .insert_header(("X-GitHub-Delivery", fixture["name"].as_str().unwrap()))
                            .insert_header((
                                "X-Hub-Signature-256",
                                format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
                            ))
                            .set_payload(wire.clone())
                            .to_request(),
                    )
                    .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let ack: Value = test::read_body_json(response).await;
                    assert_eq!(ack["data"]["duplicate"], duplicate);
                    if duplicate {
                        assert_eq!(ack["data"]["messageId"], message_id);
                    }
                    message_id = ack["data"]["messageId"].clone();
                }
                let path = format!("/api/messages/{}", message_id.as_str().unwrap());
                let detail = api!(get, &path, json!({})).1;
                let mut expected = fixture["payload"].clone();
                expected.as_object_mut().unwrap().remove("future_metadata");
                assert_eq!(detail["platformData"]["event"]["payload"], expected);
                assert!(detail["platformDataError"].is_null());
                assert_eq!(
                    detail["metadata"]["deduplicationKey"]["scope"],
                    "github:delivery"
                );
                let updated = api!(patch, &path, json!({"pinned":true,"status":"read"})).1;
                assert_eq!(updated["platformData"], detail["platformData"]);
                let response = test::call_service(
                    &app,
                    test::TestRequest::get()
                        .uri(&format!("{path}/raw"))
                        .insert_header(("Authorization", format!("Bearer {token}")))
                        .to_request(),
                )
                .await;
                let original: Value = test::read_body_json(response).await;
                assert_eq!(
                    serde_json::from_str::<Value>(original["data"]["payload"].as_str().unwrap())
                        .unwrap(),
                    fixture["payload"]
                );
            }
        }
        if provider == "stripe" {
            let fixtures: Value = serde_json::from_str(include_str!(
                "../../nomail-core/src/webhook/platforms/stripe/fixtures/deliveries.json"
            ))
            .unwrap();
            for fixture in fixtures.as_array().unwrap() {
                let body = fixture["body"].to_string();
                let time = Utc::now().timestamp().to_string();
                let mut mac = Hmac::<Sha256>::new_from_slice(b"whsec_platform-secret").unwrap();
                mac.update(format!("{time}.{body}").as_bytes());
                let signature = format!("t={time},v1={}", hex::encode(mac.finalize().into_bytes()));
                let mut message_id = Value::Null;
                for duplicate in [false, true] {
                    let response = test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri(receive_path)
                            .insert_header(("Content-Type", "application/json"))
                            .insert_header(("Stripe-Signature", signature.as_str()))
                            .set_payload(body.clone())
                            .to_request(),
                    )
                    .await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let ack: Value = test::read_body_json(response).await;
                    assert_eq!(ack["data"]["duplicate"], duplicate);
                    if duplicate {
                        assert_eq!(ack["data"]["messageId"], message_id);
                    }
                    message_id = ack["data"]["messageId"].clone();
                }
                let path = format!("/api/messages/{}", message_id.as_str().unwrap());
                let detail = api!(get, &path, json!({})).1;
                assert_eq!(
                    detail["platformData"]["event"]["payload"],
                    fixture["projection"]
                );
                assert_eq!(
                    detail["platformDataError"].is_null(),
                    !fixture["projection"].is_null()
                );
                assert_eq!(
                    detail["metadata"]["deduplicationKey"]["scope"],
                    "stripe:event"
                );
                let updated = api!(patch, &path, json!({"pinned":true,"status":"read"})).1;
                assert_eq!(updated["platformData"], detail["platformData"]);
                let response = test::call_service(
                    &app,
                    test::TestRequest::get()
                        .uri(&format!("{path}/raw"))
                        .insert_header(("Authorization", format!("Bearer {token}")))
                        .to_request(),
                )
                .await;
                let raw: Value = test::read_body_json(response).await;
                assert_eq!(
                    serde_json::from_str::<Value>(raw["data"]["payload"].as_str().unwrap())
                        .unwrap(),
                    fixture["body"]
                );
                assert!(
                    raw["data"]["header"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|h| h["name"] != "stripe-signature")
                );
            }
        }
        if provider == "telegram" {
            let unmodeled = json!({"future_envelope":{"keep":true},"update_id":null});
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(receive_path)
                    .insert_header(("X-Telegram-Bot-Api-Secret-Token", "platform-secret"))
                    .set_json(&unmodeled)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let accepted: Value = test::read_body_json(response).await;
            let unmodeled_path = format!(
                "/api/messages/{}",
                accepted["data"]["messageId"].as_str().unwrap()
            );
            let detail = api!(get, &unmodeled_path, json!({})).1;
            assert!(detail["platformData"].is_null());
            assert!(detail["platformDataError"].is_string());
            assert_eq!(
                api!(
                    patch,
                    &unmodeled_path,
                    json!({"pinned":true,"status":"read"})
                )
                .0,
                StatusCode::OK
            );
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("{unmodeled_path}/raw"))
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(
                serde_json::from_str::<Value>(
                    test::read_body_json::<Value, _>(response).await["data"]["payload"]
                        .as_str()
                        .unwrap()
                )
                .unwrap(),
                unmodeled
            );
            assert_eq!(api!(delete, &unmodeled_path, json!({})).0, StatusCode::OK);
            let fixtures: Vec<Value> = serde_json::from_str(include_str!(
                "../../nomail-core/src/webhook/platforms/telegram/fixtures/updates.json"
            ))
            .unwrap();
            for fixture in fixtures {
                let mut payload = fixture.clone();
                payload["future_metadata"] = json!({"retained":true});
                let response = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(receive_path)
                        .insert_header(("X-Telegram-Bot-Api-Secret-Token", "platform-secret"))
                        .set_json(&payload)
                        .to_request(),
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let ack: Value = test::read_body_json(response).await;
                assert_eq!(ack["data"]["duplicate"], false);
                let path = format!(
                    "/api/messages/{}",
                    ack["data"]["messageId"].as_str().unwrap()
                );
                let detail = api!(get, &path, json!({})).1;
                assert_eq!(detail["platformData"]["event"]["payload"], fixture);
                assert_eq!(
                    detail["metadata"]["deduplicationKey"]["scope"],
                    "telegram:bot-update"
                );
                let original = test::call_service(
                    &app,
                    test::TestRequest::get()
                        .uri(&format!("{path}/raw"))
                        .insert_header(("Authorization", format!("Bearer {token}")))
                        .to_request(),
                )
                .await;
                let original: Value = test::read_body_json(original).await;
                assert_eq!(
                    serde_json::from_str::<Value>(original["data"]["payload"].as_str().unwrap())
                        .unwrap(),
                    payload
                );
                assert!(
                    original["data"]["header"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|h| {
                            !nomail_app::webhook::is_raw_header_blocked(h["name"].as_str().unwrap())
                        })
                );
                // Deduplication remains scoped to update_id, not message_id or
                // the presence of a typed branch / future metadata.
                let replay = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(receive_path)
                        .insert_header(("X-Telegram-Bot-Api-Secret-Token", "platform-secret"))
                        .set_json(&fixture)
                        .to_request(),
                )
                .await;
                let replay: Value = test::read_body_json(replay).await;
                assert_eq!(replay["data"]["duplicate"], true);
                assert_eq!(replay["data"]["messageId"], ack["data"]["messageId"]);
            }
        }
        if provider == "gitea" {
            let detail = api!(get, &message_path, json!({})).1;
            assert_eq!(detail["platformData"]["provider"], "gitea");
            assert_eq!(
                detail["platformData"]["event"]["payload"]["sender"]["id"],
                -2
            );
            assert_eq!(
                detail["platformData"]["event"]["payload"]["issue"]["is_locked"],
                true
            );
            assert_eq!(
                detail["platformData"]["event"]["header"]["event_type"],
                "issue_assign"
            );
            assert_eq!(
                detail["metadata"]["deduplicationKey"]["scope"],
                "gitea:delivery"
            );
            // A form transport of the same delivery is a duplicate; signing is
            // over the decoded JSON. Manual Gitea Replay uses a new delivery ID.
            for (delivery, duplicate) in [("matrix", true), ("matrix-replay", false)] {
                let mut request = test::TestRequest::post().uri(receive_path);
                for (name, value) in &headers {
                    if !name.eq_ignore_ascii_case("x-gitea-delivery") {
                        request = request.insert_header((name.as_str(), value.as_str()));
                    }
                }
                let response = test::call_service(
                    &app,
                    request
                        .insert_header(("X-Gitea-Delivery", delivery))
                        .set_form([("payload", raw.as_str())])
                        .to_request(),
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let acknowledgement: Value = test::read_body_json(response).await;
                assert_eq!(acknowledgement["data"]["duplicate"], duplicate);
                assert_eq!(
                    acknowledgement["data"]["messageId"] == received["data"]["messageId"],
                    duplicate
                );
            }
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("{message_path}/raw"))
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            let original: Value = test::read_body_json(response).await;
            assert_eq!(
                serde_json::from_str::<Value>(original["data"]["payload"].as_str().unwrap())
                    .unwrap(),
                serde_json::from_str::<Value>(&raw).unwrap()
            );
        }
        if provider == "appstoreconnect" {
            let detail = api!(get, &message_path, json!({})).1;
            assert_eq!(detail["platformData"]["provider"], "appstoreconnect");
            let data = &detail["platformData"]["event"]["payload"]["data"];
            assert_eq!(data["type"], "appStoreVersionAppVersionStateUpdated");
            assert_eq!(data["attributes"]["oldValue"], "PREPARE_FOR_SUBMISSION");
            assert_eq!(data["attributes"]["newValue"], "READY_FOR_REVIEW");
            assert_eq!(
                data["relationships"]["instance"]["data"]["id"],
                "example-version"
            );
            assert_eq!(detail["metadata"]["version"], "1");
            assert_eq!(
                detail["metadata"]["deduplicationKey"]["scope"],
                "appstoreconnect:event"
            );
            let mut next_event: Value = serde_json::from_str(&raw).unwrap();
            next_event["data"]["id"] = json!("matrix-next-event");
            // Same notification deduplicates; another event concerning the same
            // app version must create a new message, not collide on instance ID.
            for (body, duplicate) in [(raw.clone(), true), (next_event.to_string(), false)] {
                let mut mac = Hmac::<Sha256>::new_from_slice(b"platform-secret").unwrap();
                mac.update(body.as_bytes());
                let response = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri(receive_path)
                        .insert_header(("Content-Type", "application/json"))
                        .insert_header((
                            "X-Apple-Signature",
                            format!("hmacsha256={}", hex::encode(mac.finalize().into_bytes())),
                        ))
                        .set_payload(body)
                        .to_request(),
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let acknowledgement: Value = test::read_body_json(response).await;
                assert_eq!(acknowledgement["data"]["duplicate"], duplicate);
                assert_eq!(
                    acknowledgement["data"]["messageId"] == received["data"]["messageId"],
                    duplicate
                );
            }
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("{message_path}/raw"))
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(
                serde_json::from_str::<Value>(
                    test::read_body_json::<Value, _>(response).await["data"]["payload"]
                        .as_str()
                        .unwrap()
                )
                .unwrap(),
                serde_json::from_str::<Value>(&raw).unwrap()
            );
        }
        // Missing authentication never falls back to a weaker mode.
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(receive_path)
                .set_json(json!({"title":"unauthenticated"}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{provider}");
        // A partial platform replacement cannot reuse credentials from storage.
        assert_eq!(
            api!(
                patch,
                &config_path,
                json!({"platform":{"provider":"github"}})
            )
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(api!(get, &config_path, json!({})).1, created);
        // Switching provider replaces provider + ciphertext in one mutation.
        let replacement = json!({"provider":"generic","credentials":{"type":"none"}});
        let updated = api!(
            patch,
            &config_path,
            json!({"platform":replacement,"description":""})
        );
        assert_eq!(updated.0, StatusCode::OK);
        assert_eq!(updated.1["platform"]["authentication"], "none");
        assert_eq!(updated.1["key"], created["key"]);
        assert_eq!(
            api!(post, receive_path, json!({"title":"explicit anonymous"})).0,
            StatusCode::OK
        );
        assert_eq!(api!(delete, &config_path, json!({})).0, StatusCode::OK);
    }
    database.close().await.unwrap();
}

fn platform_delivery(provider: &str, platform: &Value) -> (String, Vec<(String, String)>) {
    let time = Utc::now().timestamp().to_string();
    let body = match provider {
        "generic" => json!({"id":"matrix","title":"Test","source":null,"content":""}),
        "github" => json!({"action":"opened","issue":{"title":"Test","assignee":null,"labels":[],"locked":false}}),
        "gitea" => json!({"action":"assigned","issue":{"number":1,"title":"Test","is_locked":true,"milestone":null,"labels":[]},"sender":{"id":-2,"login":"actions"}}),
        "appstoreconnect" => json!({"data":{"type":"appStoreVersionAppVersionStateUpdated","id":"matrix","version":1,"attributes":{"oldValue":"PREPARE_FOR_SUBMISSION","newValue":"READY_FOR_REVIEW","timestamp":null},"relationships":{"instance":{"data":{"type":"appStoreVersions","id":"example-version"}}}},"future":{"keep":true}}),
        "gitlab" => json!({"object_kind":"issue","project":null,"commits":[],"object_attributes":{"title":"Test","milestone_id":null}}),
        "stripe" => json!({"id":"evt_matrix","object":"event","type":"payment_intent.succeeded","request":null,"livemode":false,"pending_webhooks":0,"data":{"object":{"id":"pi_test"}}}),
        "slack" => json!({"type":"event_callback","event_id":"EvMatrix","team_id":null,"event":{"type":"message","text":"Test","user":null}}),
        "feishu" => json!({"schema":"2.0","header":{"token":"platform-secret","event_id":"matrix","event_type":"im.message.receive_v1","tenant_key":null},"event":{"message":{"message_type":"text","content":"{\"text\":\"Test\"}","parent_id":null},"sender":null}}),
        "telegram" => json!({"update_id":123,"message":{"message_id":1,"chat":{"id":1,"type":"private"},"text":"Test","from":null,"entities":[]}}),
        "shopify" => json!({"id":123,"title":"Test","name":null}),
        _ => unreachable!(),
    }.to_string();
    let sign = |key: &[u8], input: &str| {
        let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
        mac.update(input.as_bytes());
        mac.finalize().into_bytes()
    };
    let mut headers = match provider {
        "generic" => vec![(
            "X-NoMail-Signature",
            format!("sha256={}", hex::encode(sign(b"platform-secret", &body))),
        )],
        "github" => vec![
            ("X-GitHub-Event", "issues".into()),
            ("X-GitHub-Delivery", "matrix".into()),
            (
                "X-Hub-Signature-256",
                format!("sha256={}", hex::encode(sign(b"platform-secret", &body))),
            ),
        ],
        "gitea" => vec![
            ("X-Gitea-Event", "issues".into()),
            ("X-Gitea-Event-Type", "issue_assign".into()),
            ("X-Gitea-Delivery", "matrix".into()),
            (
                "X-Gitea-Signature",
                hex::encode(sign(b"platform-secret", &body)),
            ),
        ],
        "appstoreconnect" => vec![(
            "X-Apple-Signature",
            format!(
                "hmacsha256={}",
                hex::encode(sign(b"platform-secret", &body))
            ),
        )],
        "gitlab" if platform["credentials"]["type"] == "signingToken" => vec![
            ("X-Gitlab-Event", "Issue Hook".into()),
            ("webhook-id", "matrix".into()),
            ("webhook-timestamp", time.clone()),
            (
                "webhook-signature",
                format!(
                    "v1,{}",
                    STANDARD.encode(sign(b"platform-secret", &format!("matrix.{time}.{body}")))
                ),
            ),
        ],
        "gitlab" => vec![
            ("X-Gitlab-Event", "Issue Hook".into()),
            ("X-Gitlab-Token", "platform-secret".into()),
        ],
        "stripe" => vec![(
            "Stripe-Signature",
            format!(
                "t={time},v1={}",
                hex::encode(sign(b"whsec_platform-secret", &format!("{time}.{body}")))
            ),
        )],
        "slack" => vec![
            ("X-Slack-Request-Timestamp", time.clone()),
            (
                "X-Slack-Signature",
                format!(
                    "v0={}",
                    hex::encode(sign(b"platform-secret", &format!("v0:{time}:{body}")))
                ),
            ),
        ],
        "feishu" => vec![],
        "telegram" => vec![("X-Telegram-Bot-Api-Secret-Token", "platform-secret".into())],
        "shopify" => vec![
            ("X-Shopify-Topic", "products/create".into()),
            ("X-Shopify-Shop-Domain", "test.myshopify.com".into()),
            ("X-Shopify-API-Version", "2026-10".into()),
            (
                "X-Shopify-Hmac-Sha256",
                STANDARD.encode(sign(b"platform-secret", &body)),
            ),
        ],
        _ => unreachable!(),
    };
    headers.extend([
        ("Authorization", "Bearer raw-private-credential".into()),
        ("Cookie", "session=raw-private-credential".into()),
        ("X-Api-Key", "raw-private-credential".into()),
        ("X-Raw-Unknown", "retained".into()),
        ("X-Raw-Empty", "".into()),
        ("X-Raw-Repeated", "one".into()),
        ("X-Raw-Repeated", "two".into()),
    ]);
    (
        body,
        headers.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    )
}
