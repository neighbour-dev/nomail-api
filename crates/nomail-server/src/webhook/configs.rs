//! HTTP adaptation for the shared configuration use cases.
mod repository;
use crate::{
    AppState,
    auth::Auth,
    error::{ApiResult, success},
};
use actix_web::web;
use nomail_app::{
    http::{PaginationQuery, endpoints as e},
    webhook::config::*,
};

struct Runtime<'a>(&'a [u8; 32]);
impl WebhookConfigRuntime for Runtime<'_> {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
    fn random_bytes(&self, bytes: &mut [u8]) -> ConfigResult<()> {
        use rand::RngCore;
        rand::rngs::OsRng
            .try_fill_bytes(bytes)
            .map_err(|e| ConfigError::Internal(e.to_string()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        self.0
    }
}
fn service(state: &AppState) -> WebhookConfigService<repository::Store<'_>, Runtime<'_>> {
    WebhookConfigService {
        store: repository::Store(&state.db),
        runtime: Runtime(&state.config.encryption_key),
    }
}
pub async fn list(
    state: web::Data<AppState>,
    auth: Auth,
    query: web::Query<PaginationQuery>,
) -> ApiResult {
    Ok(success::<e::ListConfigs>(
        service(&state)
            .list(auth.user_id, query.into_inner())
            .await?
            .into(),
    ))
}
pub async fn create(
    state: web::Data<AppState>,
    auth: Auth,
    body: web::Json<ConfigRequest>,
) -> ApiResult {
    Ok(success::<e::CreateConfig>(
        service(&state)
            .create(auth.user_id, body.into_inner())
            .await?
            .into(),
    ))
}
pub async fn get(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    Ok(success::<e::GetConfig>(
        service(&state).get(auth.user_id, &path).await?.into(),
    ))
}
pub async fn update(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<ConfigRequest>,
) -> ApiResult {
    Ok(success::<e::UpdateConfig>(
        service(&state)
            .update(auth.user_id, &path, body.into_inner())
            .await?
            .into(),
    ))
}
pub async fn enabled(
    state: web::Data<AppState>,
    auth: Auth,
    path: web::Path<String>,
    body: web::Json<EnabledRequest>,
) -> ApiResult {
    Ok(success::<e::EnableConfig>(
        service(&state)
            .enabled(auth.user_id, &path, body.enabled)
            .await?
            .into(),
    ))
}
pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    service(&state).delete(auth.user_id, &path).await?;
    Ok(success::<e::DeleteConfig>(()))
}
