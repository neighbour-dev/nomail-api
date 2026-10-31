//! Native composition and execution capabilities. No endpoint binding lives here.
use crate::{AppState, auth, message, webhook};
use chrono::{DateTime, Utc};
use nomail_app::{
    auth::{AuthError, AuthResult, AuthRuntime},
    http::{Environment, EnvironmentServices, HealthCheck, HttpResult, RequestHead, Services},
    ingestion::{IngestionResult, IngestionRuntime},
    webhook::config::{ConfigError, ConfigResult, WebhookConfigRuntime},
};

pub(crate) struct NativeEnvironment<'a>(pub &'a AppState);
impl<'a> Environment for NativeEnvironment<'a> {
    type AuthStore = auth::repository::Store<'a>;
    type MessageStore = message::repository::Store<'a>;
    type ConfigStore = webhook::configs::repository::Store<'a>;
    type IngestionStore = message::repository::Store<'a>;
    type Runtime = Runtime;

    fn services(&self) -> HttpResult<EnvironmentServices<Self>> {
        let state = self.0;
        Ok(Services::new(
            auth::repository::Store(&state.db),
            message::repository::Store(&state.db),
            webhook::configs::repository::Store(&state.db),
            message::repository::Store(&state.db),
            Runtime {
                encryption_key: state.config.encryption_key,
            },
            auth::auth_config(&state.config),
        ))
    }
    async fn health(&self, request: &RequestHead) -> HttpResult<HealthCheck> {
        if !self
            .0
            .health_limiter
            .allow(request.metadata.ip.parse().ok())
        {
            return Ok(HealthCheck::Throttled { retry_after: 2 });
        }
        Ok(HealthCheck::Database(self.0.db.ping().await.is_ok()))
    }
}

#[derive(Clone)]
pub(crate) struct Runtime {
    encryption_key: [u8; 32],
}
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn random_bytes(&self, bytes: &mut [u8]) -> AuthResult<()> {
        auth::runtime::Runtime.random_bytes(bytes)
    }
    async fn hash_password(&self, value: &str, salt: [u8; 16]) -> AuthResult<String> {
        auth::runtime::Runtime.hash_password(value, salt).await
    }
    async fn verify_password(&self, value: &str, hash: &str) -> AuthResult<bool> {
        auth::runtime::Runtime.verify_password(value, hash).await
    }
}
impl WebhookConfigRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        AuthRuntime::now(self)
    }
    fn random_bytes(&self, bytes: &mut [u8]) -> ConfigResult<()> {
        AuthRuntime::random_bytes(self, bytes)
            .map_err(|e: AuthError| ConfigError::Internal(e.to_string()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}
impl IngestionRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        AuthRuntime::now(self)
    }
    fn message_id(&self, _now: DateTime<Utc>) -> IngestionResult<uuid::Uuid> {
        Ok(uuid::Uuid::now_v7())
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}
