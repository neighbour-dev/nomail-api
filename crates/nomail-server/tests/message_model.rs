use actix_web::{App, http::StatusCode, test, web};
use chrono::DateTime;
use hmac::{Hmac, Mac};
use nomail_server::{AppState, app, auth, config::Config, db, entity::message, realtime::Hub};
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::{Value, json};
use sha2::Sha256;

#[actix_web::test]
async fn one_message_preserves_platform_detail_level_raw_and_independent_user_state() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: directory.path().join("nomail.db").to_str().unwrap().into(),
        jwt_secret: "test-message-model-secret".into(),
        jwt_issuer: "test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    };
    let database = db::connect(&config).await.unwrap();
    auth::ensure_initial_user(&database, &config).await.unwrap();
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
    });
    let app = test::init_service(App::new().app_data(state).configure(app::configure)).await;
    let response = test::call_service(&app,test::TestRequest::post().uri("/api/auth/login").set_json(json!({"username":"admin","password":"password123","deviceId":"a","platform":"ios","deviceType":"phone"})).to_request()).await;
    let login: Value = test::read_body_json(response).await;
    let authorization = (
        "Authorization",
        format!("Bearer {}", login["data"]["accessToken"].as_str().unwrap()),
    );
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/webhook/configs")
            .insert_header(authorization.clone())
            .set_json(json!({"name":"GitHub","platform":{"provider":"github","credentials":{"secret":"secret"}}}))
            .to_request(),
    )
    .await;
    let config: Value = test::read_body_json(response).await;
    let receive_path = config["data"]["url"].as_str().unwrap();
    let config_path = format!("/api/webhook/configs/{}", config["data"]["id"]);
    let payload = json!({"action":"created","repository":{"id":1,"full_name":"owner/repo","private":true,"created_at":"2026-01-01T00:00:00Z"},
        "issue":{"id":2,"number":123,"title":"Login failure","body":"Detailed issue body","state":"open","labels":[{"id":3,"name":"bug","color":"ff0000"}],"user":{"login":"alice","avatar_url":"https://example.com/alice.png"},"assignees":[{"id":4,"login":"bob","type":"User"}],"pull_request":{"html_url":"https://example.com/pull/123"}},
        "comment":{"id":456,"body":"Reply text","created_at":"2000-01-01T00:00:00Z","updated_at":"2000-01-02T00:00:00Z","user":{"id":4,"login":"bob"}},
        "custom":{"not_in_summary":true}});
    let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
    mac.update(payload.to_string().as_bytes());
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    let long_key = format!("{}a", "x".repeat(300));
    let other_key = format!("{}b", "x".repeat(300));
    let mut created_ids = Vec::new();
    for delivery in [&long_key, &other_key, &long_key] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(receive_path)
                .insert_header(("X-GitHub-Event", "issue_comment"))
                .insert_header(("X-GitHub-Delivery", delivery.as_str()))
                .insert_header(("X-Hub-Signature-256", signature.clone()))
                .set_json(&payload)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["data"]["duplicate"], created_ids.len() == 2);
        created_ids.push(body["data"]["messageId"].as_str().unwrap().to_owned());
    }
    assert_ne!(created_ids[0], created_ids[1]);
    assert_eq!(created_ids[0], created_ids[2]);
    let path = format!("/api/messages/{}", created_ids[0]);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&path)
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let before: Value = test::read_body_json(response).await;
    let before = &before["data"];
    assert_eq!(before["level"], "normal");
    assert_eq!(before["provider"], "github");
    assert_eq!(before["metadata"]["eventType"], "issue_comment");
    assert_eq!(before["metadata"]["deduplicationKey"]["value"], long_key);
    assert_eq!(
        before["platformData"]["event"]["payload"]["issue"]["number"],
        123
    );
    assert_eq!(before["platformData"]["event"]["payload"]["issue"]["id"], 2);
    assert_eq!(
        before["platformData"]["event"]["payload"]["issue"]["assignees"][0]["type"],
        "User"
    );
    assert_eq!(
        before["platformData"]["event"]["payload"]["issue"]["pull_request"]["html_url"],
        "https://example.com/pull/123"
    );
    assert_eq!(
        before["platformData"]["event"]["payload"]["repository"]["private"],
        true
    );
    assert_eq!(
        before["platformData"]["event"]["payload"]["repository"]["created_at"],
        "2026-01-01T00:00:00Z"
    );
    assert_eq!(
        before["platformData"]["event"]["payload"]["comment"]["updated_at"],
        "2000-01-02T00:00:00Z"
    );
    assert_eq!(
        before["platformData"]["event"]["payload"]["issue"]["labels"][0]["name"],
        "bug"
    );
    assert_eq!(
        before["platformData"]["event"]["payload"]["comment"]["user"]["login"],
        "bob"
    );
    assert_ne!(before["createdAt"], "2000-01-01T00:00:00Z");

    let response = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&path)
            .insert_header(authorization.clone())
            .set_json(json!({"status":"read","pinned":true}))
            .to_request(),
    )
    .await;
    let after: Value = test::read_body_json(response).await;
    assert_eq!(after["data"]["status"], "read");
    assert_eq!(after["data"]["pinned"], true);
    assert_eq!(after["data"]["platformData"], before["platformData"]);
    assert_eq!(after["data"]["metadata"], before["metadata"]);
    assert_eq!(after["data"]["createdAt"], before["createdAt"]);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{path}/raw"))
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    assert_eq!(
        response.headers().get("Content-Type").unwrap(),
        "application/json"
    );
    let raw: Value = test::read_body_json(response).await;
    assert_eq!(raw, payload);

    let workflow = json!({"action":"completed","workflow_run":{"id":10,"name":"CI","status":"completed","conclusion":"failure","head_branch":"main","run_number":11,"display_title":"Build PR","run_attempt":2,"actor":{"id":4,"login":"bob"}}});
    let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
    mac.update(workflow.to_string().as_bytes());
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(receive_path)
            .insert_header(("X-GitHub-Event", "workflow_run"))
            .insert_header(("X-GitHub-Delivery", "ci-failure"))
            .insert_header((
                "X-Hub-Signature-256",
                format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
            ))
            .set_json(&workflow)
            .to_request(),
    )
    .await;
    let workflow: Value = test::read_body_json(response).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!(
                "/api/messages/{}",
                workflow["data"]["messageId"].as_str().unwrap()
            ))
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let workflow: Value = test::read_body_json(response).await;
    assert_eq!(workflow["data"]["level"], "normal");
    assert_eq!(
        workflow["data"]["platformData"]["event"]["payload"]["workflow_run"]["run_attempt"],
        2
    );
    assert_eq!(
        workflow["data"]["platformData"]["event"]["payload"]["workflow_run"]["actor"]["id"],
        4
    );
    assert_eq!(
        workflow["data"]["platformData"]["event"]["payload"]["workflow_run"]["conclusion"],
        "failure"
    );
    assert_eq!(
        workflow["data"]["platformData"]["event"]["payload"]["workflow_run"]["head_branch"],
        "main"
    );
    let received =
        DateTime::parse_from_rfc3339(workflow["data"]["createdAt"].as_str().unwrap()).unwrap();
    let expiry =
        DateTime::parse_from_rfc3339(workflow["data"]["expireAt"].as_str().unwrap()).unwrap();
    assert_eq!((expiry - received).num_days(), 3);

    // Changing an endpoint affects future inputs, never past message snapshots.
    let response = test::call_service(
        &app,
        test::TestRequest::patch()
            .uri(&config_path)
            .insert_header(authorization.clone())
            .set_json(json!({"platform":{"provider":"generic","credentials":{"type":"hmacSha256","secret":"secret"}}}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let generic = json!({"id":long_key,"title":"Generic with same ID"});
    let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
    mac.update(generic.to_string().as_bytes());
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(receive_path)
            .insert_header((
                "X-NoMail-Signature",
                format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
            ))
            .set_json(&generic)
            .to_request(),
    )
    .await;
    let generic: Value = test::read_body_json(response).await;
    assert_eq!(generic["data"]["duplicate"], false);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&path)
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let snapshot: Value = test::read_body_json(response).await;
    assert_eq!(snapshot["data"]["provider"], "github");
    assert_eq!(snapshot["data"]["platformData"], before["platformData"]);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/messages")
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let list: Value = test::read_body_json(response).await;
    assert_eq!(list["data"]["total"], 4);
    for item in list["data"]["items"].as_array().unwrap() {
        assert!(item["level"].is_string());
        assert!(item.get("platformData").is_none());
        assert!(item.get("raw_payload").is_none());
    }
    for level in ["low", "normal", "high", "urgent"] {
        let payload = json!({"id":format!("priority-{level}"),"title":"Priority","level":level});
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(payload.to_string().as_bytes());
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(receive_path)
                .insert_header((
                    "X-NoMail-Signature",
                    format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
                ))
                .set_json(&payload)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let received: Value = test::read_body_json(response).await;
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/messages/{}",
                    received["data"]["messageId"].as_str().unwrap()
                ))
                .insert_header(authorization.clone())
                .to_request(),
        )
        .await;
        let detail: Value = test::read_body_json(response).await;
        assert_eq!(detail["data"]["level"], level);
        let start =
            DateTime::parse_from_rfc3339(detail["data"]["createdAt"].as_str().unwrap()).unwrap();
        let expiry =
            DateTime::parse_from_rfc3339(detail["data"]["expireAt"].as_str().unwrap()).unwrap();
        assert_eq!((expiry - start).num_days(), 3);
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/messages?level={level}"))
                .insert_header(authorization.clone())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let list: Value = test::read_body_json(response).await;
        let items = list["data"]["items"].as_array().unwrap();
        assert!(items.iter().all(|item| item["level"] == level));
        assert!(
            items
                .iter()
                .any(|item| item["id"] == received["data"]["messageId"])
        );
    }
    for old_level in ["success", "warning", "critical", "error"] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/messages?level={old_level}"))
                .insert_header(authorization.clone())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            database
                .execute_unprepared(&format!("UPDATE notify_messages SET level='{old_level}'"))
                .await
                .is_err()
        );
    }
    let records = message::Entity::find().all(&database).await.unwrap();
    assert!(
        records
            .iter()
            .any(|m| m.deduplication_key.as_deref() == Some(&long_key))
    );
    for sql in [
        "UPDATE notify_messages SET level='unknown'",
        "UPDATE notify_messages SET pinned=2",
        "UPDATE notify_messages SET deduplication_scope=NULL",
        "UPDATE notify_messages SET raw_headers='{}'",
        "UPDATE notify_messages SET webhook_config_id=999",
    ] {
        assert!(
            database.execute_unprepared(sql).await.is_err(),
            "constraint missing: {sql}"
        );
    }
    database.close().await.unwrap();
}
