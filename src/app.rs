use crate::{
    AppState, auth,
    error::{ApiError, success},
    message, push, webhook,
};
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use serde_json::json;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(
        web::JsonConfig::default()
            .limit(4 * 1024 * 1024)
            .error_handler(|error, _| ApiError::bad(error.to_string()).into()),
    )
    .app_data(web::PayloadConfig::new(4 * 1024 * 1024))
    .app_data(
        web::QueryConfig::default()
            .error_handler(|error, _| ApiError::bad(error.to_string()).into()),
    )
    .service(
        web::scope("/api")
            .configure(auth::routes)
            .configure(webhook::routes)
            .configure(message::routes)
            .configure(push::routes)
            .route("/health", web::get().to(health)),
    )
    .default_service(web::to(|| async {
        ApiError::not_found("not found").error_response()
    }));
}
#[derive(Clone, Default)]
pub struct HealthLimiter(Arc<Mutex<HashMap<Option<IpAddr>, Instant>>>);

impl HealthLimiter {
    fn allow(&self, ip: Option<IpAddr>) -> bool {
        let now = Instant::now();
        let mut entries = self.0.lock().expect("health limiter lock");
        entries.retain(|_, time| now.duration_since(*time) < Duration::from_secs(2));
        if entries.contains_key(&ip) {
            return false;
        }
        entries.insert(ip, now);
        true
    }
}

async fn health(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if !state
        .health_limiter
        .allow(req.peer_addr().map(|addr| addr.ip()))
    {
        return HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", "2"))
            .body("Too Many Requests");
    }
    let status = if state.db.ping().await.is_ok() {
        "ok"
    } else {
        "error"
    };
    success(json!({"status": status, "db": status}))
}
