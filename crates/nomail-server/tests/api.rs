//! End-to-end contracts against an isolated, temporary SQLite file.
use actix_web::{App, http::StatusCode, test, web};
use chrono::Utc;
use nomail_server::{
    AppState, app, auth,
    config::Config,
    db,
    entity::{user, webhook},
    realtime::Hub,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};

#[actix_web::test]
async fn all_api_contracts() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: directory.path().join("nomail.db").to_str().unwrap().into(),
        jwt_secret: "test-secret-at-least-32-characters".into(),
        jwt_issuer: "nomail-test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    };
    let database = db::connect(&config).await.unwrap();
    auth::ensure_initial_user(&database, &config).await.unwrap();
    auth::ensure_initial_user(&database, &config).await.unwrap();
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
    });
    let app =
        test::init_service(App::new().app_data(state.clone()).configure(app::configure)).await;

    macro_rules! api {
        ($method:ident, $url:expr, $body:expr, $token:expr) => {{
            let mut request = test::TestRequest::$method().uri($url);
            let token: Option<&str> = $token;
            if let Some(token) = token {
                request = request.insert_header(("Authorization", format!("Bearer {token}")));
            }
            let response = test::call_service(&app, request.set_json($body).to_request()).await;
            let status = response.status();
            let body: Value = test::read_body_json(response).await;
            (status, body)
        }};
    }
    let (status, body) = api!(get, "/api/messages", json!({}), None);
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], 401);
    assert!(body["data"].is_null());
    let invalid = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/login")
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{")
            .to_request(),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(invalid).await;
    assert_eq!(body["code"], 400);
    let (status, _) = api!(
        post,
        "/api/auth/login",
        json!({"username":"admin","password":"wrong","deviceId":"a","platform":"ios","deviceType":"phone"}),
        None
    );
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, login) = api!(
        post,
        "/api/auth/login",
        json!({"username":"admin","password":"password123","deviceId":"a","platform":"ios","deviceType":"phone","deviceName":"iPhone"}),
        None
    );
    assert_eq!(status, StatusCode::OK, "{login}");
    assert_eq!(login["data"]["user"]["requiresAccountSetup"], true);
    let token = login["data"]["accessToken"].as_str().unwrap().to_owned();
    let refresh = login["data"]["refreshToken"].as_str().unwrap().to_owned();
    let claims = nomail_app::auth::token::decode(
        &nomail_app::auth::AuthConfig {
            jwt_secret: state.config.jwt_secret.clone(),
            jwt_issuer: state.config.jwt_issuer.clone(),
            initial_username: state.config.initial_username.clone(),
            initial_password: state.config.initial_password.clone(),
        },
        &token,
        Utc::now(),
    )
    .unwrap();
    assert_eq!(claims.user_id, 1);
    let (_, body) = api!(get, "/api/auth/account", json!({}), Some(&token));
    assert_eq!(body["data"]["username"], "admin");
    let (_, body) = api!(get, "/api/auth/devices", json!({}), Some(&token));
    assert_eq!(body["data"][0]["deviceId"], "a");
    assert_eq!(body["data"][0]["loggedIn"], true);
    assert!(body["data"][0].get("pushEnabled").is_none());
    for (method, path) in [
        ("POST", "/api/push/devices"),
        ("PATCH", "/api/push/devices/a"),
        ("DELETE", "/api/push/devices/a"),
    ] {
        let request = test::TestRequest::default()
            .method(method.parse().unwrap())
            .uri(path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    let (status, created) = api!(
        post,
        "/api/webhook/configs/",
        json!({"name":"Monitor","platform":{"provider":"generic","credentials":{"type":"none"}},"description":"test"}),
        Some(&token)
    );
    assert_eq!(status, StatusCode::OK, "{created}");
    let config_id = created["data"]["id"].as_i64().unwrap();
    let config_path = format!("/api/webhook/configs/{config_id}");
    let receive_path = created["data"]["url"].as_str().unwrap();
    let (_, body) = api!(
        get,
        "/api/webhook/configs?pageSize=999",
        json!({}),
        Some(&token)
    );
    assert_eq!(body["data"]["pageSize"], 100);
    let (_, body) = api!(get, &config_path, json!({}), Some(&token));
    assert_eq!(body["data"]["name"], "Monitor");
    let (_, body) = api!(patch, &config_path, json!({"name":"Updated"}), Some(&token));
    assert_eq!(body["data"]["name"], "Updated");
    let (status, body) = api!(
        post,
        receive_path,
        json!({"id":"e1","title":"Disk","content":"hello\n 世界","level":"urgent"}),
        None
    );
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["duplicate"], false);
    let message_id = body["data"]["messageId"].as_str().unwrap().to_owned();
    assert_eq!(
        uuid::Uuid::parse_str(&message_id)
            .unwrap()
            .get_version_num(),
        7
    );
    let (_, body) = api!(post, receive_path, json!({"id":"e1","title":"Disk"}), None);
    assert_eq!(body["data"]["duplicate"], true);
    assert_eq!(body["data"]["messageId"], message_id);
    let (_, body) = api!(get, "/api/messages/", json!({}), Some(&token));
    assert_eq!(body["data"]["total"], 1);
    assert_eq!(body["data"]["items"][0]["summary"], "hello 世界");
    assert_eq!(body["data"]["items"][0]["level"], "urgent");
    let (_, body) = api!(get, "/api/messages/unread-count", json!({}), Some(&token));
    assert_eq!(body["data"]["count"], 1);
    let (status, _) = api!(get, "/api/messages?from=bad", json!({}), Some(&token));
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, body) = api!(
        get,
        "/api/messages?query=%E4%B8%96%E7%95%8C&status=unread&level=urgent&pinned=false",
        json!({}),
        Some(&token)
    );
    assert_eq!(body["data"]["total"], 1);
    let message_path = format!("/api/messages/{message_id}");
    let (_, body) = api!(get, &message_path, json!({}), Some(&token));
    assert_eq!(body["data"]["webhookConfig"]["name"], "Updated");
    assert_eq!(body["data"]["hasRawPayload"], true);
    let raw = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("{message_path}/raw"))
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(raw.status(), StatusCode::OK);
    assert_eq!(
        raw.headers().get("Content-Type").unwrap(),
        "application/json"
    );
    let raw_body: Value = test::read_body_json(raw).await;
    assert_eq!(raw_body["code"], 0);
    assert_eq!(raw_body["msg"], "success");
    assert_eq!(raw_body["data"]["contentType"], "application/json");
    assert_eq!(
        serde_json::from_str::<Value>(raw_body["data"]["payload"].as_str().unwrap()).unwrap()["id"],
        "e1"
    );
    let (_, body) = api!(
        patch,
        &message_path,
        json!({"status":"read","pinned":true}),
        Some(&token)
    );
    assert_eq!(body["data"]["pinned"], true);
    assert!(!body["data"]["readAt"].is_null());
    let (_, body) = api!(
        post,
        "/api/messages/batch",
        json!({"ids":[message_id,message_id],"action":"mark_unread"}),
        Some(&token)
    );
    assert_eq!(body["data"]["affected"], 1);
    let (_, body) = api!(get, &message_path, json!({}), Some(&token));
    assert!(body["data"]["readAt"].is_null());
    for action in ["pin", "unpin", "mark_read"] {
        let (status, body) = api!(
            post,
            "/api/messages/batch",
            json!({"ids":[message_id],"action":action}),
            Some(&token)
        );
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    // Tenant filtering must apply even though public registration is disabled.
    let now = Utc::now();
    let other = user::ActiveModel {
        username: Set("other".into()),
        password_hash: Set("unused".into()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    let other_config = webhook::ActiveModel {
        user_id: Set(other.id),
        name: Set("private".into()),
        key: Set("private-key".into()),
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
    let (_, body) = api!(
        post,
        "/api/webhook/private-key",
        json!({"id":"private","content":"private"}),
        None
    );
    let private_id = body["data"]["messageId"].as_str().unwrap();
    let (status, _) = api!(
        get,
        &format!("/api/messages/{private_id}"),
        json!({}),
        Some(&token)
    );
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api!(
        get,
        &format!("/api/webhook/configs/{}", other_config.id),
        json!({}),
        Some(&token)
    );
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, body) = api!(
        post,
        "/api/messages/batch",
        json!({"ids":[private_id],"action":"delete"}),
        Some(&token)
    );
    assert_eq!(body["data"]["affected"], 0);

    // SSE only notifies; the message is already queryable when the notification arrives.
    use actix_web::body::{BoxBody, MessageBody};
    use std::future::poll_fn;
    use std::pin::Pin;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/messages/stream")
            .insert_header(("Authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(
        response.headers().get("Content-Type").unwrap(),
        "text/event-stream"
    );
    let mut stream: BoxBody = response.into_body();
    let first = poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
        .await
        .unwrap()
        .unwrap();
    assert!(
        std::str::from_utf8(&first)
            .unwrap()
            .contains("event: ready")
    );
    let (_, body) = api!(
        post,
        receive_path,
        json!({"id":"e2","content":"event"}),
        None
    );
    let second_id = body["data"]["messageId"].as_str().unwrap().to_owned();
    let event = poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
        .await
        .unwrap()
        .unwrap();
    let (status, detail) = api!(
        get,
        &format!("/api/messages/{second_id}"),
        json!({}),
        Some(&token)
    );
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["data"]["content"], "event");
    let notification = json!({"id": second_id, "receivedAt": detail["data"]["createdAt"]});
    let event = std::str::from_utf8(&event).unwrap();
    assert!(event.starts_with(&format!("id: {second_id}\nevent: message.created\ndata: ")));
    assert!(event.ends_with("\n\n"));
    let data: serde_json::Value = serde_json::from_str(
        event
            .lines()
            .nth(2)
            .unwrap()
            .strip_prefix("data: ")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(data, notification);
    drop(stream);

    let short_claims = nomail_app::auth::Claims {
        user_id: claims.user_id,
        session_id: claims.session_id,
        sub: claims.sub.clone(),
        iss: claims.iss.clone(),
        iat: Utc::now().timestamp(),
        exp: Utc::now().timestamp() + 2,
    };
    let short_token = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &short_claims,
        &jsonwebtoken::EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .unwrap();
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/messages/stream")
            .insert_header(("Authorization", format!("Bearer {short_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut stream = response.into_body();
    let _ready = poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
        .await
        .unwrap()
        .unwrap();
    let expired = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert!(
        std::str::from_utf8(&expired)
            .unwrap()
            .contains("event: auth.expired")
    );
    assert!(
        poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
            .await
            .is_none()
    );
    drop(stream);

    // Deleted messages retain their idempotency key.
    let (status, _) = api!(delete, &message_path, json!({}), Some(&token));
    assert_eq!(status, StatusCode::OK);
    let (_, body) = api!(
        post,
        receive_path,
        json!({"id":"e1","content":"again"}),
        None
    );
    assert_eq!(body["data"]["messageId"], message_id);
    assert_eq!(body["data"]["duplicate"], true);
    let (_, body) = api!(
        post,
        "/api/messages/batch",
        json!({"ids":[second_id],"action":"delete"}),
        Some(&token)
    );
    assert_eq!(body["data"]["affected"], 1);
    let (_, body) = api!(
        patch,
        &format!("{config_path}/enabled"),
        json!({"enabled":false}),
        Some(&token)
    );
    assert_eq!(body["data"]["enabled"], false);
    let (status, _) = api!(post, receive_path, json!({"content":"disabled"}), None);
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api!(delete, &config_path, json!({}), Some(&token));
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api!(get, &config_path, json!({}), Some(&token));
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Concurrent refresh must only accept the original token once.
    let request_one = test::TestRequest::post()
        .uri("/api/auth/refresh-token")
        .insert_header(("User-Agent", "nomail-refresh-agent"))
        .insert_header(("CF-Connecting-IP", "100.101.102.103"))
        .insert_header(("X-Forwarded-For", "192.168.1.20, 9.9.9.9"))
        .peer_addr("127.0.0.2:1234".parse().unwrap())
        .set_json(json!({"refreshToken": refresh}))
        .to_request();
    let request_two = test::TestRequest::post()
        .uri("/api/auth/refresh-token")
        .insert_header(("User-Agent", "nomail-refresh-agent"))
        .insert_header(("CF-Connecting-IP", "100.101.102.103"))
        .insert_header(("X-Forwarded-For", "192.168.1.20, 9.9.9.9"))
        .peer_addr("127.0.0.2:1234".parse().unwrap())
        .set_json(json!({"refreshToken": refresh}))
        .to_request();
    let (one, two) = tokio::join!(
        test::call_service(&app, request_one),
        test::call_service(&app, request_two)
    );
    assert!(
        (one.status() == StatusCode::OK && two.status() == StatusCode::UNAUTHORIZED)
            || (two.status() == StatusCode::OK && one.status() == StatusCode::UNAUTHORIZED)
    );
    let valid = if one.status() == StatusCode::OK {
        one
    } else {
        two
    };
    let valid: Value = test::read_body_json(valid).await;
    let (_, devices) = api!(get, "/api/auth/devices", json!({}), Some(&token));
    let device = devices["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["deviceId"] == "a")
        .unwrap();
    assert_eq!(device["userAgent"], "nomail-refresh-agent");
    assert_eq!(device["ipAddress"], "100.101.102.103");
    let new_refresh = valid["data"]["refreshToken"].as_str().unwrap();
    let (status, _) = api!(
        post,
        "/api/auth/logout",
        json!({"refreshToken":new_refresh}),
        None
    );
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api!(get, "/api/auth/account", json!({}), Some(&token));
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Four devices leave only the newest three active.
    let mut tokens = Vec::new();
    for id in ["a", "b", "c", "d"] {
        let (status, body) = api!(
            post,
            "/api/auth/login",
            json!({"username":"admin","password":"password123","deviceId":id,"platform":"ios","deviceType":"phone"}),
            None
        );
        assert_eq!(status, StatusCode::OK, "{body}");
        tokens.push(body["data"]["accessToken"].as_str().unwrap().to_owned());
    }
    let (status, _) = api!(get, "/api/auth/account", json!({}), Some(&tokens[0]));
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let token = &tokens[3];
    let (status, _) = api!(
        delete,
        "/api/auth/devices/b/session",
        json!({}),
        Some(token)
    );
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api!(get, "/api/auth/account", json!({}), Some(&tokens[1]));
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = api!(delete, "/api/auth/devices/b", json!({}), Some(token));
    assert_eq!(status, StatusCode::OK);
    let (status, body) = api!(
        patch,
        "/api/auth/account",
        json!({"currentPassword":"password123","username":"renamed"}),
        Some(token)
    );
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["username"], "renamed");
    let (status, _) = api!(
        patch,
        "/api/auth/account",
        json!({"currentPassword":"password123","newPassword":"changed456"}),
        Some(token)
    );
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api!(get, "/api/auth/account", json!({}), Some(token));
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    auth::reset_account(&database, None, "reset789pass".into())
        .await
        .unwrap();
    let (status, body) = api!(
        post,
        "/api/auth/login",
        json!({"username":"renamed","password":"reset789pass","deviceId":"a","platform":"web","deviceType":"web"}),
        None
    );
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, health) = api!(get, "/api/health", json!({}), None);
    assert_eq!(health["data"]["status"], "ok");
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/api/health").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Re-running baseline migration never drops existing data.
    use sea_orm_migration::MigratorTrait;
    nomail_server::migration::Migrator::up(&database, None)
        .await
        .unwrap();
    assert!(
        user::Entity::find_by_id(1)
            .one(&database)
            .await
            .unwrap()
            .is_some()
    );
    database.close().await.unwrap();
}
