//! End-to-end contract tests. Use only a disposable PostgreSQL database:
//! TEST_DATABASE_URL=postgres://... cargo test --test api -- --ignored
use actix_web::{App, http::StatusCode, middleware::NormalizePath, test, web};
use chrono::Utc;
use nomail_api::{
    AppState, app, auth,
    config::Config,
    crypto, db,
    entity::{user, webhook},
    realtime::Hub,
};
use sea_orm::{ActiveModelTrait, Database, EntityTrait, Set};
use serde_json::{Value, json};

#[actix_web::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn all_api_contracts() {
    let base = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let schema = format!("nomail_test_{}", crypto::random_token(8));
    let admin = Database::connect(&base).await.unwrap();
    use sea_orm::ConnectionTrait;
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut url = url::Url::parse(&base).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let config = Config {
        database_url: url.into(),
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
        health_limiter: app::HealthLimiter::default(),
    });
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(NormalizePath::trim())
            .configure(app::configure),
    )
    .await;

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
    let claims = auth::decode_token(&token, &state).unwrap();
    assert_eq!(claims.user_id, 1);
    let (_, body) = api!(get, "/api/auth/account", json!({}), Some(&token));
    assert_eq!(body["data"]["username"], "admin");
    let (_, body) = api!(get, "/api/auth/devices", json!({}), Some(&token));
    assert_eq!(body["data"][0]["deviceId"], "a");
    assert_eq!(body["data"][0]["loggedIn"], true);
    assert_eq!(body["data"][0]["pushEnabled"], false);

    let (status, body) = api!(
        post,
        "/api/push/devices/",
        json!({"deviceId":"a","provider":"apns","pushToken":"token-a"}),
        Some(&token)
    );
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["environment"], "production");
    let (_, body) = api!(
        patch,
        "/api/push/devices/a",
        json!({"enabled":false}),
        Some(&token)
    );
    assert_eq!(body["data"]["enabled"], false);
    let (_, body) = api!(
        post,
        "/api/push/devices",
        json!({"deviceId":"a","provider":"fcm","pushToken":"new-token"}),
        Some(&token)
    );
    assert_eq!(body["data"]["enabled"], true);
    let endpoints = nomail_api::entity::push_endpoint::Entity::find()
        .all(&database)
        .await
        .unwrap();
    assert_eq!(
        crypto::decrypt(&endpoints[0].encrypted_token, &[7; 32]).unwrap(),
        "new-token"
    );
    let (status, _) = api!(delete, "/api/push/devices/a", json!({}), Some(&token));
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api!(delete, "/api/push/devices/a", json!({}), Some(&token));
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, created) = api!(
        post,
        "/api/webhook/configs/",
        json!({"name":"Monitor","provider":" Generic ","description":"test"}),
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
        json!({"id":"e1","title":"Disk","content":"hello\n 世界","level":"error"}),
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
    assert_eq!(body["data"]["items"][0]["level"], "critical");
    let (_, body) = api!(get, "/api/messages/unread-count", json!({}), Some(&token));
    assert_eq!(body["data"]["count"], 1);
    let (status, _) = api!(get, "/api/messages?from=bad", json!({}), Some(&token));
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, body) = api!(
        get,
        "/api/messages?query=%E4%B8%96%E7%95%8C&status=unread&level=critical&pinned=false",
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
    assert_eq!(raw_body["id"], "e1");
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

    // SSE is authenticated, sends ready immediately and publishes newly persisted messages.
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
    assert!(
        std::str::from_utf8(&event)
            .unwrap()
            .contains("message.created")
    );
    drop(stream);

    let short_claims = auth::Claims {
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
        .set_json(json!({"refreshToken": refresh}))
        .to_request();
    let request_two = test::TestRequest::post()
        .uri("/api/auth/refresh-token")
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
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // Re-running baseline migration never drops existing data.
    use sea_orm_migration::MigratorTrait;
    nomail_api::migration::Migrator::up(&database, None)
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
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    admin.close().await.unwrap();
}

#[actix_web::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn adopts_legacy_schema_password_tokens_and_ciphertext() {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let base = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let schema = format!("nomail_legacy_{}", crypto::random_token(8));
    let admin = Database::connect(&base).await.unwrap();
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut url = url::Url::parse(&base).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let legacy = Database::connect(url.as_str()).await.unwrap();
    legacy
        .execute_unprepared(include_str!("fixtures/legacy-schema.sql"))
        .await
        .unwrap();
    let fixture: Value = serde_json::from_str(include_str!("fixtures/legacy-crypto.json")).unwrap();
    let hash = fixture["passwordHash"].as_str().unwrap();
    let encrypted = fixture["encryptedSecret"].as_str().unwrap();
    legacy.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO users (username,password_hash,created_at,updated_at) VALUES ('legacy',$1,now(),now())", [hash.into()])).await.unwrap();
    legacy.execute_unprepared("INSERT INTO devices (user_id,device_id,created_at,updated_at) VALUES (1,'legacy-device',now(),now());
        INSERT INTO sessions (device_id,refresh_token_hash,expires_at,logged_in_at,created_at,updated_at)
        VALUES (1,'legacy-hash','2100-01-01',now(),now(),now())").await.unwrap();
    legacy.execute(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO webhook_configs (user_id,name,key,provider,encrypted_secret,created_at,updated_at) VALUES (1,'legacy','legacy-key','generic',$1,now(),now())", [encrypted.into()])).await.unwrap();
    legacy
        .execute_unprepared(
            "INSERT INTO notify_messages (id,webhook_config_id,title,created_at,updated_at)
        VALUES ('0198c000-0000-7000-8000-000000000001',1,'legacy message',now(),now())",
        )
        .await
        .unwrap();
    legacy.close().await.unwrap();

    let config = Config {
        database_url: url.into(),
        jwt_secret: "test-secret-at-least-32-characters".into(),
        jwt_issuer: "nomail-test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: String::new(),
        initial_password: String::new(),
    };
    let database = db::connect(&config).await.unwrap();
    auth::ensure_initial_user(&database, &config).await.unwrap();
    let state = web::Data::new(AppState {
        db: database.clone(),
        config,
        realtime: Hub::default(),
        health_limiter: app::HealthLimiter::default(),
    });
    let old_access = fixture["accessToken"].as_str().unwrap();
    assert_eq!(
        auth::decode_token(old_access, &state).unwrap().session_id,
        1
    );
    assert_eq!(
        crypto::decrypt(encrypted, &[7; 32]).unwrap(),
        "legacy-secret 中文"
    );
    crypto::verify_password("legacy123pass".into(), hash.into())
        .await
        .unwrap();
    let app =
        test::init_service(App::new().app_data(state.clone()).configure(app::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/messages")
            .insert_header(("Authorization", format!("Bearer {old_access}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["data"]["items"][0]["title"], "legacy message");
    assert_eq!(body["data"]["items"][0]["source"], "");
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/devices")
            .insert_header(("Authorization", format!("Bearer {old_access}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["data"][0]["deviceName"], "");
    let response = test::call_service(&app, test::TestRequest::post().uri("/api/auth/login")
        .set_json(json!({"username":"legacy","password":"legacy123pass","deviceId":"legacy-device","platform":"ios","deviceType":"phone"})).to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    // Receiving with an old encrypted secret verifies both stored data and HMAC interop.
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let body = br#"{"id":"legacy-new","content":"received"}"#;
    let mut mac = Hmac::<Sha256>::new_from_slice("legacy-secret 中文".as_bytes()).unwrap();
    mac.update(body);
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/webhook/legacy-key")
            .insert_header(("X-NoMail-Signature", signature))
            .set_payload(body.as_slice())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["data"]["duplicate"], false);
    database.close().await.unwrap();
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    admin.close().await.unwrap();
}
