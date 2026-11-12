use actix_web::{App, http::StatusCode, test, web};
use chrono::{Duration, Utc};
use nomail_server::{
    AppState, app, auth,
    config::Config,
    db,
    entity::{message, user, webhook},
    realtime::Hub,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};
use uuid::Uuid;

#[actix_web::test]
async fn sparse_updates_bulk_limits_scope_and_raw_reads_use_the_shared_service() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: dir.path().join("messages.db").to_str().unwrap().into(),
        jwt_secret: "test-message-service-secret".into(),
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
    macro_rules! api {
        ($method:ident,$path:expr,$body:expr,$token:expr) => {{
            let token: Option<&str> = $token;
            let mut request = test::TestRequest::$method().uri($path);
            if let Some(token) = token {
                request = request.insert_header(("Authorization", format!("Bearer {token}")));
            }
            let response = test::call_service(&app, request.set_json($body).to_request()).await;
            let status = response.status();
            let body: Value = test::read_body_json(response).await;
            (status, body)
        }};
    }
    let (_, login) = api!(
        post,
        "/api/auth/login",
        json!({"username":"admin","password":"password123","deviceId":"a","platform":"ios","deviceType":"phone"}),
        None
    );
    let token = login["data"]["accessToken"].as_str().unwrap();
    let (_, config) = api!(
        post,
        "/api/webhook/configs",
        json!({"name":"Own","platform":{"provider":"generic","credentials":{"type":"none"}}}),
        Some(token)
    );
    let receive = config["data"]["url"].as_str().unwrap();
    let (_, first) = api!(
        post,
        receive,
        json!({"id":"first","title":"One","content":"body","level":"high","extra":{"keep":true}}),
        None
    );
    let first = first["data"]["messageId"].as_str().unwrap().to_owned();
    let (status, filtered) = api!(
        get,
        "/api/messages?status=%20unread%20&level=HIGH&pinned=F&query=%20body%20&page=0&pageSize=999",
        json!({}),
        Some(token)
    );
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["data"]["total"], 1);
    assert_eq!(filtered["data"]["page"], 1);
    assert_eq!(filtered["data"]["pageSize"], 100);
    let path = format!("/api/messages/{first}");
    let (_, snapshot) = api!(get, &path, json!({}), Some(token));
    let snapshot = snapshot["data"].clone();
    let (_, second) = api!(post, receive, json!({"id":"second","title":"Two"}), None);
    let second = second["data"]["messageId"].as_str().unwrap().to_owned();
    for _ in 0..6 {
        let (status, _) = api!(
            patch,
            &path,
            json!({"status":"unread","pinned":false}),
            Some(token)
        );
        assert_eq!(status, StatusCode::OK);
        let read = test::TestRequest::patch()
            .uri(&path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(json!({"status":"read"}))
            .to_request();
        let pin = test::TestRequest::patch()
            .uri(&path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(json!({"pinned":true}))
            .to_request();
        let (read, pin) = tokio::join!(
            test::call_service(&app, read),
            test::call_service(&app, pin)
        );
        assert_eq!(read.status(), StatusCode::OK);
        assert_eq!(pin.status(), StatusCode::OK);
        let (_, after) = api!(get, &path, json!({}), Some(token));
        let after = &after["data"];
        assert_eq!(after["status"], "read");
        assert_eq!(after["pinned"], true);
        assert!(after["readAt"].is_string());
        for key in [
            "title",
            "content",
            "source",
            "level",
            "provider",
            "metadata",
            "platformData",
            "createdAt",
            "expireAt",
        ] {
            assert_eq!(after[key], snapshot[key], "immutable field changed: {key}");
        }
        let read_at = after["readAt"].clone();
        let (_, pin_only) = api!(
            patch,
            &path,
            json!({"pinned":false,"level":"urgent"}),
            Some(token)
        );
        assert_eq!(pin_only["data"]["readAt"], read_at);
        assert_eq!(pin_only["data"]["level"], "high");
    }
    let now = Utc::now();
    let other = user::ActiveModel {
        username: Set("other".into()),
        password_hash: Set("not-used".into()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    webhook::ActiveModel {
        user_id: Set(other.id),
        name: Set("Foreign".into()),
        key: Set("foreign-key".into()),
        provider: Set("generic".into()),
        encrypted_platform: Set(nomail_app::security::encrypt(
            r#"{"provider":"generic","credentials":{"type":"none"}}"#,
            &[7; 32],
            &[0; 12],
        )
        .unwrap()),
        enabled: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    let (_, foreign) = api!(
        post,
        "/api/webhook/foreign-key",
        json!({"id":"foreign","title":"Other user's message"}),
        None
    );
    let foreign = foreign["data"]["messageId"].as_str().unwrap().to_owned();
    for suffix in ["", "/raw"] {
        assert_eq!(
            api!(
                get,
                &format!("/api/messages/{foreign}{suffix}"),
                json!({}),
                Some(token)
            )
            .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        api!(
            patch,
            &format!("/api/messages/{foreign}"),
            json!({"pinned":true}),
            Some(token)
        )
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api!(
            delete,
            &format!("/api/messages/{foreign}"),
            json!({}),
            Some(token)
        )
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, deleted) = api!(
        post,
        receive,
        json!({"id":"deleted","title":"Deleted"}),
        None
    );
    let deleted = deleted["data"]["messageId"].as_str().unwrap().to_owned();
    assert_eq!(
        api!(
            delete,
            &format!("/api/messages/{deleted}"),
            json!({}),
            Some(token)
        )
        .0,
        StatusCode::OK
    );
    let (_, result) = api!(
        post,
        "/api/messages/batch",
        json!({"ids":[first,first,second,foreign,deleted,Uuid::from_u128(99).to_string()],"action":"mark_read"}),
        Some(token)
    );
    assert_eq!(result["data"]["affected"], 2);
    let (_, one) = api!(get, &path, json!({}), Some(token));
    let (_, two) = api!(
        get,
        &format!("/api/messages/{second}"),
        json!({}),
        Some(token)
    );
    assert_eq!(one["data"]["readAt"], two["data"]["readAt"]);
    let foreign_row = message::Entity::find_by_id(Uuid::parse_str(&foreign).unwrap())
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(foreign_row.status, "unread");
    assert!(!foreign_row.pinned);
    let deleted_row = message::Entity::find_by_id(Uuid::parse_str(&deleted).unwrap())
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deleted_row.status, "unread");
    assert!(deleted_row.deleted_at.is_some());
    let (_, list) = api!(get, "/api/messages", json!({}), Some(token));
    assert_eq!(list["data"]["total"], 2);
    let mut ids = vec![first.clone()];
    ids.extend((1..100).map(|n| Uuid::from_u128(n).to_string()));
    let (status, result) = api!(
        post,
        "/api/messages/batch",
        json!({"ids":ids,"action":"pin"}),
        Some(token)
    );
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["data"]["affected"], 1);
    let before = message::Entity::find_by_id(Uuid::parse_str(&first).unwrap())
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    for body in [
        json!({"ids":[first,"invalid"],"action":"unpin"}),
        json!({"ids":vec![first.clone();101],"action":"unpin"}),
        json!({"ids":[],"action":"pin"}),
        json!({"ids":[first],"action":"invalid"}),
        json!({"ids":null,"action":"pin"}),
        json!({"ids":[first],"action":null}),
    ] {
        assert_eq!(
            api!(post, "/api/messages/batch", body, Some(token)).0,
            StatusCode::BAD_REQUEST
        );
    }
    let after = message::Entity::find_by_id(before.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after, before);
    let mut expired: message::ActiveModel = after.clone().into();
    expired.expire_at = Set(Some(Utc::now() - Duration::days(1)));
    expired.update(&database).await.unwrap();
    assert_eq!(api!(get, &path, json!({}), Some(token)).0, StatusCode::OK);
    // Raw inspection does not require successful platform-model reconstruction.
    let mut damaged: message::ActiveModel = after.into();
    damaged.provider = Set("telegram".into()); // raw has no required update_id
    damaged.update(&database).await.unwrap();
    let invalid_before = message::Entity::find_by_id(before.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        api!(
            patch,
            &path,
            json!({"pinned":!invalid_before.pinned,"status":"unread"}),
            Some(token)
        )
        .0,
        StatusCode::OK
    );
    let updated = message::Entity::find_by_id(before.id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.pinned, !invalid_before.pinned);
    assert_eq!(updated.raw_payload, invalid_before.raw_payload);
    let detail = api!(get, &path, json!({}), Some(token)).1;
    assert!(detail["data"]["platformData"].is_null());
    assert!(detail["data"]["platformDataError"].is_string());
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{path}/raw"))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let raw: Value = test::read_body_json(response).await;
    assert_eq!(raw["code"], 0);
    assert_eq!(raw["msg"], "success");
    assert_eq!(raw["data"]["payload"], invalid_before.raw_payload);
    assert_eq!(raw["data"]["contentType"], invalid_before.content_type);
    // Even malformed JSON / an unknown serialization type must be inspectable.
    let mut opaque: message::ActiveModel = updated.into();
    opaque.raw_payload = Set(" {not JSON\n".into());
    opaque.content_type = Set("application/custom; version=1".into());
    let headers =
        json!([{"name":"x-stored","value":" value ","encoding":null,"future":{"keep":[]}}]);
    opaque.raw_headers = Set(headers.to_string());
    opaque.update(&database).await.unwrap();
    let (status, opaque) = api!(get, &format!("{path}/raw"), json!({}), Some(token));
    assert_eq!(status, StatusCode::OK);
    assert_eq!(opaque["data"]["payload"], " {not JSON\n");
    assert_eq!(opaque["data"]["header"], headers);
    assert_eq!(
        opaque["data"]["contentType"],
        "application/custom; version=1"
    );
    assert_eq!(
        api!(
            delete,
            &format!("/api/webhook/configs/{}", config["data"]["id"]),
            json!({}),
            Some(token)
        )
        .0,
        StatusCode::OK
    );
    assert_eq!(
        api!(get, &path, json!({}), Some(token)).0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api!(get, "/api/messages", json!({}), Some(token)).1["data"]["total"],
        0
    );
    assert_eq!(
        api!(post, receive, json!({"id":"first","title":"again"}), None).0,
        StatusCode::NOT_FOUND
    );
    database.close().await.unwrap();
}
