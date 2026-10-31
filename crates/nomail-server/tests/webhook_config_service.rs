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
async fn config_service_preserves_sparse_concurrent_updates_credentials_and_scope() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: dir.path().join("configs.db").to_str().unwrap().into(),
        jwt_secret: "config-service-test-secret".into(),
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
        health_limiter: app::HealthLimiter::default(),
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
        json!({"name":"Original","provider":"generic","secret":"original-secret","source":"original-source","description":"original-description"})
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
        nomail_core::security::decrypt(initial.encrypted_secret.as_deref().unwrap(), &[7; 32])
            .unwrap(),
        "original-secret"
    );
    assert_eq!(api!(patch, &path, json!({})).1, created);
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
        let source = test::TestRequest::patch()
            .uri(&path)
            .insert_header(("Authorization", format!("Bearer {token}")))
            .set_json(json!({"source":format!("Source {i}")}))
            .to_request();
        let (rename, source) = tokio::join!(
            test::call_service(&app, rename),
            test::call_service(&app, source)
        );
        assert_eq!(rename.status(), StatusCode::OK);
        assert_eq!(source.status(), StatusCode::OK);
        let (_, current) = api!(get, &path, json!({}));
        assert_eq!(current["name"], format!("Name {i}"));
        assert_eq!(current["source"], format!("Source {i}"));
        assert_eq!(current["key"], created["key"]);
        assert_eq!(current["description"], created["description"]);
        let row = webhook::Entity::find_by_id(id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.encrypted_secret, initial.encrypted_secret);
        assert_eq!(row.created_at, initial.created_at);
    }
    let before = webhook::Entity::find_by_id(id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    api!(
        patch,
        &path,
        json!({"name":"","source":"","secret":"","provider":"","description":""})
    );
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
    api!(patch, &path, json!({"secret":"replacement-secret"}));
    let replaced = webhook::Entity::find_by_id(id)
        .one(&database)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(replaced.encrypted_secret, initial.encrypted_secret);
    assert_eq!(
        nomail_core::security::decrypt(replaced.encrypted_secret.as_deref().unwrap(), &[7; 32])
            .unwrap(),
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
    database.close().await.unwrap();
}
