//! Composition of existing use cases, not a second trait listing every endpoint.
use super::{HttpResult, RequestHead};
use crate::{
    auth::{AuthConfig, AuthRuntime, AuthService, AuthStore},
    ingestion::{IngestionRuntime, IngestionService, IngestionStore},
    message::{MessageService, MessageStore},
    webhook::config::{WebhookConfigRuntime, WebhookConfigService, WebhookConfigStore},
};

pub struct Services<A, M, C, I, R> {
    pub auth: AuthService<A, R>,
    pub messages: MessageService<M>,
    pub configs: WebhookConfigService<C, R>,
    pub ingestion: IngestionService<I, R>,
}
pub type EnvironmentServices<E> = Services<
    <E as Environment>::AuthStore,
    <E as Environment>::MessageStore,
    <E as Environment>::ConfigStore,
    <E as Environment>::IngestionStore,
    <E as Environment>::Runtime,
>;
impl<A, M, C, I, R: Clone> Services<A, M, C, I, R> {
    pub fn new(
        auth_store: A,
        message_store: M,
        config_store: C,
        ingestion_store: I,
        runtime: R,
        auth_config: AuthConfig,
    ) -> Self {
        Self {
            auth: AuthService {
                store: auth_store,
                runtime: runtime.clone(),
                config: auth_config,
            },
            messages: MessageService {
                store: message_store,
            },
            configs: WebhookConfigService {
                store: config_store,
                runtime: runtime.clone(),
            },
            ingestion: IngestionService {
                store: ingestion_store,
                runtime,
            },
        }
    }
}

pub enum HealthCheck {
    Database(bool),
    Throttled { retry_after: u64 },
}
#[allow(async_fn_in_trait)]
pub trait Environment {
    type AuthStore: AuthStore;
    type MessageStore: MessageStore;
    type ConfigStore: WebhookConfigStore;
    type IngestionStore: IngestionStore;
    type Runtime: AuthRuntime + WebhookConfigRuntime + IngestionRuntime + Clone;

    /// Lazy: called only for known non-health routes, not during registration.
    fn services(&self) -> HttpResult<EnvironmentServices<Self>>;
    /// Deployment-specific health I/O/rate limits; HTTP formatting stays shared.
    async fn health(&self, request: &RequestHead) -> HttpResult<HealthCheck>;
}
