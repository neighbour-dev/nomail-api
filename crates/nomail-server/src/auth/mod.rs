//! Actix extraction and runtime composition. Business rules live in nomail-core.
pub(crate) mod account;
pub(crate) mod devices;
mod repository;
pub(crate) mod runtime;
pub(crate) mod session;

use crate::{AppState, config::Config, error::ApiError};
pub use account::{ensure_initial_user, reset_account};
use actix_web::{FromRequest, HttpRequest, dev::Payload, web};
use chrono::{DateTime, Utc};
pub use nomail_core::auth::{ACCESS_TTL, Claims, MAX_DEVICES, REFRESH_DAYS};
use nomail_core::auth::{AuthConfig, AuthContext, AuthService};
use sea_orm::DatabaseConnection;
use std::{future::Future, ops::Deref, pin::Pin};

fn auth_config(config: &Config) -> AuthConfig {
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
) -> Result<(String, DateTime<Utc>), ApiError> {
    Ok(nomail_core::auth::token::issue(
        &auth_config(&state.config),
        user_id,
        session_id,
        Utc::now(),
    )?)
}
pub fn decode_token(token: &str, state: &AppState) -> Result<Claims, ApiError> {
    Ok(nomail_core::auth::token::decode(
        &auth_config(&state.config),
        token,
        Utc::now(),
    )?)
}

pub struct Auth(pub AuthContext);
impl Deref for Auth {
    type Target = AuthContext;
    fn deref(&self) -> &AuthContext {
        &self.0
    }
}
impl FromRequest for Auth {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let state = req.app_data::<web::Data<AppState>>().cloned();
        let token = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        Box::pin(async move {
            let state = state.ok_or_else(|| ApiError::internal("missing application state"))?;
            let token = nomail_core::api::bearer_token(token.as_deref())?;
            Ok(Auth(
                service(&state.db, &state.config)
                    .authenticate(token)
                    .await?,
            ))
        })
    }
}
