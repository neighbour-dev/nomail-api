//! Composition of existing use cases, not a second trait listing every endpoint.
use super::{HttpResult, RequestHead};
use crate::{
    auth::{AuthRuntime, AuthService, AuthStore},
    device::{DeviceService, DeviceStore},
    ingestion::{IngestionRuntime, IngestionService, IngestionStore},
    message::{MessageService, MessageStore},
    webhook::config::{WebhookConfigRuntime, WebhookConfigService, WebhookConfigStore},
};

pub struct Services<A, D, M, C, I, R> {
    pub auth: AuthService<A, R>,
    pub devices: DeviceService<D>,
    pub messages: MessageService<M>,
    pub configs: WebhookConfigService<C, R>,
    pub ingestion: IngestionService<I, R>,
}
pub type EnvironmentServices<E> = Services<
    <E as Environment>::AuthStore,
    <E as Environment>::DeviceStore,
    <E as Environment>::MessageStore,
    <E as Environment>::ConfigStore,
    <E as Environment>::IngestionStore,
    <E as Environment>::Runtime,
>;
impl<A, D, M, C, I, R: Clone> Services<A, D, M, C, I, R> {
    pub fn new(
        auth: AuthService<A, R>,
        device_store: D,
        message_store: M,
        config_store: C,
        ingestion_store: I,
    ) -> Self {
        let runtime = auth.runtime.clone();
        Self {
            auth,
            devices: DeviceService {
                store: device_store,
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
}
#[allow(async_fn_in_trait)]
pub trait Environment {
    type AuthStore: AuthStore;
    type DeviceStore: DeviceStore;
    type MessageStore: MessageStore;
    type ConfigStore: WebhookConfigStore;
    type IngestionStore: IngestionStore;
    type Runtime: AuthRuntime + WebhookConfigRuntime + IngestionRuntime + Clone;

    /// Lazy: called only for known non-health routes, not during registration.
    fn services(&self) -> HttpResult<EnvironmentServices<Self>>;
    fn push_enabled(&self) -> bool {
        false
    }
    /// Deployment-specific health I/O; HTTP formatting stays shared.
    async fn health(&self, request: &RequestHead) -> HttpResult<HealthCheck>;
}
