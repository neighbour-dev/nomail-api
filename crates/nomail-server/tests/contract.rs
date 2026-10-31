use actix_web::{
    App,
    http::{Method, StatusCode},
    test, web,
};
use nomail_app::http::{
    self as api, ApiResponse, AuthRequirement, EndpointResponse, PathKind, endpoints as e,
};
use nomail_server::{AppState, app, auth, config::Config, db, realtime::Hub};
use serde_json::json;

#[actix_web::test]
async fn core_inventory_controls_routes_authentication_parameters_and_response_dtos() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: directory
            .path()
            .join("contract.db")
            .to_str()
            .unwrap()
            .into(),
        jwt_secret: "contract-test-secret".into(),
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
    for route in api::ROUTES
        .iter()
        .filter(|r| r.auth == AuthRequirement::Bearer)
    {
        let sample = match route.path_kind {
            PathKind::MessageId => "0198c000-0000-7000-8000-000000000001",
            PathKind::ConfigId => "1",
            _ => "phone",
        };
        let path = if let Some(start) = route.path.find('{') {
            let end = route.path.find('}').unwrap();
            format!(
                "{}{}{}",
                &route.path[..start],
                sample,
                &route.path[end + 1..]
            )
        } else {
            route.path.into()
        };
        let response = test::call_service(
            &app,
            test::TestRequest::default()
                .method(Method::from_bytes(route.method.as_str().as_bytes()).unwrap())
                .uri(&path)
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{} {path}",
            route.method.as_str()
        );
        let error: ApiResponse<()> = test::read_body_json(response).await;
        assert_eq!(error.code, 401);
        assert_eq!(error.msg, "unauthorized");
    }
    for (method, path) in [
        (Method::POST, "/api/health"),
        (Method::GET, "/api/messages/batch"),
        (Method::GET, "/api//messages"),
        (Method::DELETE, "/api/messages/stream"),
        (Method::GET, "/api/unknown"),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::default()
                .method(method)
                .uri(path)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        let error: ApiResponse<()> = test::read_body_json(response).await;
        assert_eq!(error.code, 404);
    }
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(api::Operation::Health.route().path)
            .to_request(),
    )
    .await;
    let health: EndpointResponse<e::Health> = test::read_body_json(response).await;
    assert_eq!(health.code, 0);
    let response=test::call_service(&app,test::TestRequest::post().uri(api::Operation::Login.route().path).set_json(json!({"username":"admin","password":"password123","deviceId":"a","platform":"ios","deviceType":"phone"})).to_request()).await;
    let login: EndpointResponse<e::Login> = test::read_body_json(response).await;
    assert_eq!(login.code, 0);
    let authorization = (
        "Authorization",
        format!("Bearer {}", login.data.access_token),
    );
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(api::Operation::CreateConfig.route().path)
            .insert_header(authorization.clone())
            .set_json(json!({"name":"Test","provider":"generic"}))
            .to_request(),
    )
    .await;
    let config: EndpointResponse<e::CreateConfig> = test::read_body_json(response).await;
    assert_eq!(config.data.url, api::webhook_url(&config.data.key));
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&config.data.url)
            .set_json(json!({"title":"Test","level":"high"}))
            .to_request(),
    )
    .await;
    let ack: EndpointResponse<e::ReceiveWebhook> = test::read_body_json(response).await;
    assert_eq!(ack.code, 0);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(api::Operation::ListMessages.route().path)
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let list: EndpointResponse<e::ListMessages> = test::read_body_json(response).await;
    assert_eq!(list.data.total, 1);
    assert_eq!(
        list.data.items[0].level,
        nomail_app::webhook::MessageLevel::High
    );
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/messages?status=%20unread%20&level=HIGH")
            .insert_header(authorization.clone())
            .to_request(),
    )
    .await;
    let normalized: EndpointResponse<e::ListMessages> = test::read_body_json(response).await;
    assert_eq!(normalized.data.total, 1);
    for (payload, content_type) in [
        (b"{".as_slice(), "application/json"),
        (b"{}".as_slice(), "text/plain"),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(api::Operation::CreateConfig.route().path)
                .insert_header(authorization.clone())
                .insert_header(("Content-Type", content_type))
                .set_payload(payload)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ApiResponse<()> = test::read_body_json(response).await;
        assert_eq!(error.code, 400);
    }
    // Decode a parameter exactly once; the registered device identifier contains
    // a slash and UTF-8, not a second path segment or a literal percent sequence.
    let response=test::call_service(&app,test::TestRequest::post().uri(api::Operation::Login.route().path).set_json(json!({"username":"admin","password":"password123","deviceId":"phone/中文","platform":"ios","deviceType":"phone"})).to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = test::call_service(
        &app,
        test::TestRequest::delete()
            .uri("/api/auth/devices/phone%2F%E4%B8%AD%E6%96%87")
            .insert_header(authorization)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    database.close().await.unwrap();
}
