use actix_web::{App, http::StatusCode, test, web};
use nomail_server::{AppState, app, config::Config, realtime::Hub};
use sea_orm::DatabaseConnection;

pub async fn initialize(db: &DatabaseConnection, config: &Config) -> Result<(), StatusCode> {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState {
                db: db.clone(),
                config: config.clone(),
                realtime: Hub::default(),
            }))
            .configure(app::configure),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/setup")
            .set_json(serde_json::json!({"username":"admin", "password":"password123"}))
            .to_request(),
    )
    .await;
    if response.status() == StatusCode::OK {
        Ok(())
    } else {
        Err(response.status())
    }
}
