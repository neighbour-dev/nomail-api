use super::Auth;
use crate::{
    AppState,
    config::Config,
    error::{ApiError, ApiResult, success},
};
use actix_web::web;
use nomail_core::api::endpoints as e;
use nomail_core::auth::AccountUpdateRequest;
use sea_orm::DatabaseConnection;

pub async fn ensure_initial_user(db: &DatabaseConnection, config: &Config) -> Result<(), ApiError> {
    Ok(super::service(db, config).ensure_initial_account().await?)
}
pub async fn reset_account(
    db: &DatabaseConnection,
    username: Option<String>,
    password: String,
) -> Result<(), ApiError> {
    // Reset needs no token-signing configuration and remains a local-only tool.
    let config = nomail_core::auth::AuthConfig {
        jwt_secret: String::new(),
        jwt_issuer: String::new(),
        initial_username: String::new(),
        initial_password: String::new(),
    };
    Ok(nomail_core::auth::AuthService {
        store: super::repository::Store(db),
        runtime: super::runtime::Runtime,
        config,
    }
    .reset_account(username, &password)
    .await?)
}
pub async fn get(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    Ok(success::<e::GetAccount>(
        super::service(&state.db, &state.config)
            .account(auth.user_id)
            .await?,
    ))
}
pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<AccountUpdateRequest>,
) -> ApiResult {
    Ok(success::<e::UpdateAccount>(
        super::service(&state.db, &state.config)
            .update_account(auth.user_id, body.into_inner())
            .await?,
    ))
}
