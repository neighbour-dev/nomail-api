//! Complete community webhook use case, independent of HTTP and persistence encoding.
mod error;
pub use error::{IngestionError, IngestionResult};

use crate::{
    message::{Acceptance, MessageService, MessageStore},
    webhook::{
        self, ProtocolResponse, Provider, ReceiveOutcome, WebhookConfig, WebhookRequest, service,
    },
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Trusted stored configuration. Never Debug/Serialize: contains credentials.
pub struct ReceivingConfig {
    pub id: i64,
    pub user_id: i64,
    pub provider: Provider,
    pub source: Option<String>,
    pub encrypted_secret: Option<String>,
}

#[allow(async_fn_in_trait)]
pub trait IngestionStore: MessageStore {
    /// Only enabled, undeleted configurations are eligible for reception.
    async fn receiving_config(&self, key: &str) -> IngestionResult<Option<ReceivingConfig>>;
}
pub trait IngestionRuntime {
    fn now(&self) -> DateTime<Utc>;
    fn message_id(&self, now: DateTime<Utc>) -> IngestionResult<Uuid>;
    fn encryption_key(&self) -> &[u8; 32];
}

pub enum IngestionOutcome {
    Control(ProtocolResponse),
    Ignored,
    Accepted {
        user_id: i64,
        acceptance: Acceptance,
    },
}

pub struct IngestionService<S, R> {
    pub messages: MessageService<S>,
    pub runtime: R,
}
impl<S: IngestionStore, R: IngestionRuntime> IngestionService<S, R> {
    pub async fn receive(
        &self,
        key: &str,
        request: &WebhookRequest<'_>,
    ) -> IngestionResult<IngestionOutcome> {
        let config = self
            .messages
            .store
            .receiving_config(key)
            .await?
            .ok_or(IngestionError::ConfigNotFound)?;
        let provider = service::configured_provider(config.provider.as_str())
            .map_err(|_| IngestionError::Internal("webhook provider is not configured".into()))?;
        let secret = match config.encrypted_secret.as_deref().filter(|s| !s.is_empty()) {
            Some(value) => {
                crate::security::decrypt(value, self.runtime.encryption_key()).map_err(|_| {
                    IngestionError::Internal("webhook credential decryption failed".into())
                })?
            }
            None => String::new(),
        };
        let now = self.runtime.now();
        let outcome = webhook::receive(
            &WebhookConfig::new(
                provider,
                service::authentication(provider, &secret),
                now.timestamp(),
            ),
            request,
        )?;
        match outcome {
            ReceiveOutcome::Respond(response) => Ok(IngestionOutcome::Control(response)),
            ReceiveOutcome::Ignored { .. } => Ok(IngestionOutcome::Ignored),
            ReceiveOutcome::Event(event) => {
                let acceptance = self
                    .messages
                    .accept(
                        config.id,
                        *event,
                        config.source.as_deref(),
                        self.runtime.message_id(now)?,
                        now,
                    )
                    .await?;
                Ok(IngestionOutcome::Accepted {
                    user_id: config.user_id,
                    acceptance,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests;
