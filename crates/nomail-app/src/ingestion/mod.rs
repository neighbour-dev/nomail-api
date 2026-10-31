//! Complete community webhook use case, independent of HTTP and persistence encoding.
mod error;
pub use error::{IngestionError, IngestionResult};

use crate::{
    message::{Acceptance, InsertOutcome, Message, MessageAccepted, MessageError, MessageResult},
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
pub trait IngestionStore {
    /// Only enabled, undeleted configurations are eligible for reception.
    async fn receiving_config(&self, key: &str) -> IngestionResult<Option<ReceivingConfig>>;
    /// Atomically insert the supplied record once. No state/TTL/default decisions.
    /// Deduplication includes configuration, scope and full key, survives soft
    /// deletion, and treats absent keys as independent deliveries.
    async fn insert_once(&self, message: &Message) -> MessageResult<InsertOutcome>;
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
    pub store: S,
    pub runtime: R,
}
impl<S: IngestionStore, R: IngestionRuntime> IngestionService<S, R> {
    pub async fn receive(
        &self,
        key: &str,
        request: &WebhookRequest<'_>,
    ) -> IngestionResult<IngestionOutcome> {
        let config = self
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
                let message = Message::from_received_event(
                    config.id,
                    *event,
                    config.source.as_deref(),
                    self.runtime.message_id(now)?,
                    now,
                )?;
                let outcome = self.store.insert_once(&message).await?;
                let created_event = match outcome {
                    InsertOutcome::Duplicate(_) => None,
                    InsertOutcome::Created(id) if id == message.id => {
                        Some(MessageAccepted::from(&message))
                    }
                    InsertOutcome::Created(_) => {
                        return Err(MessageError::Internal(
                            "insert returned an inconsistent message id".into(),
                        )
                        .into());
                    }
                };
                Ok(IngestionOutcome::Accepted {
                    user_id: config.user_id,
                    acceptance: Acceptance {
                        outcome,
                        created_event,
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests;
