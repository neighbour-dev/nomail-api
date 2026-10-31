//! Workers HTTP/runtime adaptation; configuration policies live in App.
mod repository;
use crate::{
    config::Config,
    db::Store,
    error::{Result, success},
    time,
};
use nomail_app::{
    http::{PaginationQuery, endpoints as e},
    webhook::config::*,
};
use worker::Response;

struct Runtime<'a>(&'a [u8; 32]);
impl WebhookConfigRuntime for Runtime<'_> {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(time::now_ms()).expect("runtime timestamp in range")
    }
    fn random_bytes(&self, bytes: &mut [u8]) -> ConfigResult<()> {
        getrandom::getrandom(bytes).map_err(|e| ConfigError::Internal(e.to_string()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        self.0
    }
}
fn service<'a>(
    db: &'a Store,
    config: &'a Config,
) -> WebhookConfigService<repository::Store<'a>, Runtime<'a>> {
    WebhookConfigService {
        store: repository::Store(db),
        runtime: Runtime(&config.encryption_key),
    }
}
pub async fn list(
    query: PaginationQuery,
    uid: i64,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    success::<e::ListConfigs>(service(db, config).list(uid, query).await?.into())
}
pub async fn create(
    body: ConfigRequest,
    uid: i64,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    success::<e::CreateConfig>(service(db, config).create(uid, body).await?.into())
}
pub async fn get(id: &str, uid: i64, db: &Store, config: &Config) -> Result<Response> {
    success::<e::GetConfig>(service(db, config).get(uid, id).await?.into())
}
pub async fn update(
    id: &str,
    body: ConfigRequest,
    uid: i64,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    success::<e::UpdateConfig>(service(db, config).update(uid, id, body).await?.into())
}
pub async fn delete(id: &str, uid: i64, db: &Store, config: &Config) -> Result<Response> {
    service(db, config).delete(uid, id).await?;
    success::<e::DeleteConfig>(())
}
pub async fn enabled(
    id: &str,
    body: EnabledRequest,
    uid: i64,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    success::<e::EnableConfig>(
        service(db, config)
            .enabled(uid, id, body.enabled)
            .await?
            .into(),
    )
}
