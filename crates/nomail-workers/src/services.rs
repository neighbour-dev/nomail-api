//! D1/Workers composition and execution capabilities, without endpoint bindings.
use crate::{auth, config::Config, db::Store, message, time, webhook};
use chrono::{DateTime, Utc};
use nomail_app::{
    auth::{AuthConfig, AuthResult, AuthRuntime},
    http::{
        ApiFailure, Environment, EnvironmentServices, HealthCheck, HttpResult, RequestHead,
        Services,
    },
    ingestion::{IngestionError, IngestionResult, IngestionRuntime},
    webhook::config::{ConfigError, ConfigResult, WebhookConfigRuntime},
};
use worker::Env;

pub(crate) struct WorkersEnvironment<'a> {
    pub db: &'a worker::Result<Store>,
    pub env: &'a Env,
}
impl WorkersEnvironment<'_> {
    fn database(&self) -> HttpResult<&Store> {
        self.db.as_ref().map_err(|_| ApiFailure::internal().into())
    }
}
impl<'a> Environment for WorkersEnvironment<'a> {
    type AuthStore = auth::repository::Store<'a>;
    type MessageStore = message::repository::Store<'a>;
    type ConfigStore = webhook::configs::repository::Store<'a>;
    type IngestionStore = message::repository::Store<'a>;
    type Runtime = Runtime;

    fn services(&self) -> HttpResult<EnvironmentServices<Self>> {
        let db = self.db.as_ref().map_err(|_| ApiFailure::internal())?;
        let config = Config::load(self.env).map_err(|error| error.into_http())?;
        Ok(Services::new(
            auth::repository::Store(db),
            message::repository::Store(db),
            webhook::configs::repository::Store(db),
            message::repository::Store(db),
            Runtime {
                encryption_key: config.encryption_key,
            },
            AuthConfig {
                jwt_secret: config.jwt_secret,
                jwt_issuer: config.jwt_issuer,
                initial_username: self
                    .env
                    .secret("INITIAL_USERNAME")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                initial_password: self
                    .env
                    .secret("INITIAL_PASSWORD")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            },
        ))
    }
    async fn health(&self, _request: &RequestHead) -> HttpResult<HealthCheck> {
        Ok(HealthCheck::Database(
            self.database()?.rows("SELECT 1", vec![]).await.is_ok(),
        ))
    }
}

#[derive(Clone)]
pub(crate) struct Runtime {
    encryption_key: [u8; 32],
}
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(time::now_ms()).expect("runtime timestamp in range")
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
        AuthRuntime::random_bytes(self, bytes).map_err(|e| ConfigError::Internal(e.to_string()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}
impl IngestionRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        AuthRuntime::now(self)
    }
    fn message_id(&self, now: DateTime<Utc>) -> IngestionResult<uuid::Uuid> {
        crate::crypto::message_id(now.timestamp_millis() as u64)
            .map_err(|_| IngestionError::Internal("message identity generation failed".into()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}
