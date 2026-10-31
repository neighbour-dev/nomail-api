use actix_web::{App, http::StatusCode, test, web};
use futures_util::future::join_all;
use nomail_server::{AppState, app, auth, config::Config, db, realtime::Hub};
use serde_json::{Value, json};

#[actix_web::test]
async fn concurrent_logins_and_webhook_deliveries_preserve_limits_and_uniqueness() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: directory.path().join("nomail.db").to_str().unwrap().into(),
        jwt_secret: "test-concurrency-secret".into(),
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
    let logins = join_all((0..5).map(|id| {
        test::call_service(&app,test::TestRequest::post().uri("/api/auth/login").set_json(json!({"username":"admin","password":"password123","deviceId":format!("device-{id}"),"platform":"ios","deviceType":"phone"})).to_request())
    })).await;
    for response in logins {
        assert_eq!(response.status(), StatusCode::OK);
    }
    let latest = test::call_service(&app,test::TestRequest::post().uri("/api/auth/login").set_json(json!({"username":"admin","password":"password123","deviceId":"last","platform":"ios","deviceType":"phone"})).to_request()).await;
    let latest: Value = test::read_body_json(latest).await;
    let token = latest["data"]["accessToken"].as_str().unwrap();
    let authorization = ("Authorization", format!("Bearer {token}"));
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/auth/devices")
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let devices: Value = test::read_body_json(response).await;
    assert_eq!(
        devices["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["loggedIn"] == true)
            .count(),
        3
    );
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/webhook/configs")
            .insert_header(authorization.clone())
            .set_json(json!({"name":"Test","provider":"generic"}))
            .to_request(),
    )
    .await;
    let config: Value = test::read_body_json(response).await;
    let path = config["data"]["url"].as_str().unwrap();
    let deliveries = join_all((0..8).map(|_| {
        test::call_service(
            &app,
            test::TestRequest::post()
                .uri(path)
                .set_json(json!({"id":"same-delivery","title":"Test"}))
                .to_request(),
        )
    }))
    .await;
    let mut created = 0;
    let mut id = None;
    for response in deliveries {
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        if body["data"]["duplicate"] == false {
            created += 1;
        }
        let current = body["data"]["messageId"].as_str().unwrap().to_owned();
        if let Some(previous) = &id {
            assert_eq!(previous, &current);
        }
        id = Some(current);
    }
    assert_eq!(created, 1);
    for _ in 0..2 {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(path)
                .set_json(json!({"title":"No ID"}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/messages")
            .insert_header(authorization)
            .to_request(),
    )
    .await;
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["data"]["total"], 3);
    database.close().await.unwrap();
}
