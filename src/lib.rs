pub mod app;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod entity;
pub mod error;
pub mod message;
pub mod migration;
pub mod push;
pub mod realtime;
pub mod webhook;

#[derive(Clone)]
pub struct AppState {
    pub db: sea_orm::DatabaseConnection,
    pub config: config::Config,
    pub realtime: realtime::Hub,
    pub health_limiter: app::HealthLimiter,
}
