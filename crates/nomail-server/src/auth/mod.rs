//! Native authentication storage/runtime and local account maintenance.
pub(crate) mod account;
pub(crate) mod repository;
pub(crate) mod runtime;

use crate::{AppState, config::Config};
pub use account::{ensure_initial_user, reset_account};
use chrono::{DateTime, Utc};
pub use nomail_app::auth::{ACCESS_TTL, Claims, MAX_DEVICES, REFRESH_DAYS};
use nomail_app::auth::{AuthConfig, AuthResult, AuthService};
use sea_orm::DatabaseConnection;

pub(crate) fn auth_config(config: &Config) -> AuthConfig {
    AuthConfig {
        jwt_secret: config.jwt_secret.clone(),
        jwt_issuer: config.jwt_issuer.clone(),
        initial_username: config.initial_username.clone(),
        initial_password: config.initial_password.clone(),
    }
}
pub(crate) fn service<'a>(
    db: &'a DatabaseConnection,
    config: &Config,
) -> AuthService<repository::Store<'a>, runtime::Runtime> {
    AuthService {
        store: repository::Store(db),
        runtime: runtime::Runtime,
        config: auth_config(config),
    }
}

pub fn access_token(
    state: &AppState,
    user_id: i64,
    session_id: i64,
) -> AuthResult<(String, DateTime<Utc>)> {
    nomail_app::auth::token::issue(&auth_config(&state.config), user_id, session_id, Utc::now())
}
pub fn decode_token(token: &str, state: &AppState) -> AuthResult<Claims> {
    nomail_app::auth::token::decode(&auth_config(&state.config), token, Utc::now())
}
