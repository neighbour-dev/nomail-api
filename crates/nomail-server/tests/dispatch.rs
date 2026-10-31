use actix_web::{
    App,
    http::{Method, StatusCode},
    test, web,
};
use nomail_app::http::{ApiResponse, MAX_BODY_BYTES};
use nomail_server::{AppState, app, config::Config, db, realtime::Hub};

#[actix_web::test]
async fn shared_dispatch_preserves_routing_authentication_body_order_and_health_limits() {
    let directory = tempfile::tempdir().unwrap();
    let config = Config {
        database_path: directory
            .path()
            .join("dispatch.db")
            .to_str()
            .unwrap()
            .into(),
        jwt_secret: "dispatch-test-secret".into(),
        jwt_issuer: "test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    };
    let state = web::Data::new(AppState {
        db: db::connect(&config).await.unwrap(),
        config,
        realtime: Hub::default(),
        health_limiter: app::HealthLimiter::default(),
    });
    let app = test::init_service(App::new().app_data(state).configure(app::configure)).await;
    let oversized = vec![b'x'; MAX_BODY_BYTES + 1];
    for (method, path, token, expected) in [
        (Method::POST, "/api/unknown", None, StatusCode::NOT_FOUND),
        (Method::POST, "/api/messages", None, StatusCode::NOT_FOUND),
        (
            Method::GET,
            "/api/webhook/configs/0",
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::PATCH,
            "/api/messages/not-a-uuid",
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::PATCH,
            "/api/webhook/configs/bad",
            Some("invalid"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::PATCH,
            "/api/auth/account",
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::PATCH,
            "/api/auth/account",
            Some("invalid"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Method::POST,
            "/api/auth/login",
            None,
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let mut request = test::TestRequest::default()
            .method(method)
            .uri(path)
            .insert_header(("Content-Type", "application/json"));
        if let Some(token) = token {
            request = request.insert_header(("Authorization", format!("Bearer {token}")));
        }
        let response =
            test::call_service(&app, request.set_payload(oversized.clone()).to_request()).await;
        assert_eq!(response.status(), expected, "{path}");
        let error: ApiResponse<()> = test::read_body_json(response).await;
        assert_eq!(error.code, expected.as_u16());
    }
    let response = test::call_service(&app, test::TestRequest::post().uri("/api/auth/login")
        .set_json(serde_json::json!({"username":"admin", "password":"password123", "deviceId":"paths", "platform":"ios", "deviceType":"phone"}))
        .to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let login: serde_json::Value = test::read_body_json(response).await;
    let token = login["data"]["accessToken"].as_str().unwrap();
    for (path, payload, content_type, status, message) in [
        (
            "/api/webhook/configs/bad",
            "{".to_owned(),
            "application/json",
            StatusCode::BAD_REQUEST,
            "invalid webhook config id",
        ),
        (
            "/api/webhook/configs/0/enabled",
            "{}".to_owned(),
            "text/plain",
            StatusCode::BAD_REQUEST,
            "invalid webhook config id",
        ),
        (
            "/api/messages/not-a-uuid",
            "{".to_owned(),
            "text/plain",
            StatusCode::BAD_REQUEST,
            "invalid message id",
        ),
        (
            "/api/messages/not-a-uuid",
            "x".repeat(MAX_BODY_BYTES + 1),
            "application/json",
            StatusCode::PAYLOAD_TOO_LARGE,
            "request body too large",
        ),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(path)
                .insert_header(("Authorization", format!("Bearer {token}")))
                .insert_header(("Content-Type", content_type))
                .set_payload(payload)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), status, "{path}");
        let error: ApiResponse<()> = test::read_body_json(response).await;
        assert_eq!(error.msg, message, "{path}");
    }
    // GET health does not consume a body, and the transport-only limiter still
    // produces the exact shared JSON envelope and Retry-After header.
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/health")
            .set_payload(oversized)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/api/health").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers().get("Retry-After").unwrap(), "2");
    let error: ApiResponse<()> = test::read_body_json(response).await;
    assert_eq!(error.msg, "Too Many Requests");
}
