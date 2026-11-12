//! D1/Workers service composition, without endpoint bindings.
use crate::{
    auth,
    config::{Config, ConfigLoadError},
    db::Store,
    message,
    runtime::Runtime,
    webhook,
};
use nomail_app::{
    auth::AuthConfig,
    http::{
        ApiFailure, Environment, EnvironmentServices, HealthCheck, HttpResult, RequestHead,
        Services,
    },
};
use worker::Env;

pub(crate) struct WorkersEnvironment<'a> {
    pub db: &'a worker::Result<Store>,
    pub env: &'a Env,
}
fn config_failure(error: ConfigLoadError) -> ApiFailure {
    // Preserve the public configuration failure contract at the HTTP boundary,
    // rather than teaching configuration loading about responses.
    match error {
        ConfigLoadError::EmptyJwtSecret => ApiFailure::bad("JWT_SECRET is required"),
        ConfigLoadError::InvalidEncryptionKey => {
            ApiFailure::new(500, "invalid encryption key configuration")
        }
        ConfigLoadError::MissingSecret(_) => ApiFailure::internal(),
    }
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
        let config = Config::load(self.env).map_err(config_failure)?;
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
            },
        ))
    }
    async fn health(&self, _request: &RequestHead) -> HttpResult<HealthCheck> {
        Ok(HealthCheck::Database(
            self.database()?.rows("SELECT 1", vec![]).await.is_ok(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_failure_mapping_preserves_existing_http_behavior() {
        assert_eq!(
            config_failure(ConfigLoadError::MissingSecret("JWT_SECRET")),
            ApiFailure::internal()
        );
        assert_eq!(
            config_failure(ConfigLoadError::EmptyJwtSecret),
            ApiFailure::bad("JWT_SECRET is required")
        );
        assert_eq!(
            config_failure(ConfigLoadError::InvalidEncryptionKey),
            ApiFailure::new(500, "invalid encryption key configuration")
        );
    }
}
