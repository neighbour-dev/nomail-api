//! Composition of existing use cases, not a second trait listing every endpoint.
use super::{HttpResult, RequestHead};
use crate::{
    auth::{AuthRuntime, AuthService, AuthStore},
    device::{DeviceService, DeviceStore},
    ingestion::{IngestionRuntime, IngestionService, IngestionStore},
    message::{MessageService, MessageStore},
    push::PreferenceStore,
    webhook::config::{WebhookConfigRuntime, WebhookConfigService, WebhookConfigStore},
};

pub struct Services<A, D, M, C, I, P, R> {
    pub auth: AuthService<A, R>,
    pub devices: DeviceService<D>,
    pub messages: MessageService<M>,
    pub configs: WebhookConfigService<C, R>,
    pub ingestion: IngestionService<I, R>,
    pub push_preferences: P,
}
pub type EnvironmentServices<E> = Services<
    <E as Environment>::AuthStore,
    <E as Environment>::DeviceStore,
    <E as Environment>::MessageStore,
    <E as Environment>::ConfigStore,
    <E as Environment>::IngestionStore,
    <E as Environment>::PreferenceStore,
    <E as Environment>::Runtime,
>;
impl<A, D, M, C, I, P, R: Clone> Services<A, D, M, C, I, P, R> {
    pub fn new(
        auth: AuthService<A, R>,
        device_store: D,
        message_store: M,
        config_store: C,
        ingestion_store: I,
        push_preferences: P,
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
            push_preferences,
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
    type PreferenceStore: PreferenceStore;
    type Runtime: AuthRuntime + WebhookConfigRuntime + IngestionRuntime + Clone;

    /// Lazy: called only for known non-health routes, not during registration.
    fn services(&self) -> HttpResult<EnvironmentServices<Self>>;
    fn push_enabled(&self) -> bool {
        false
    }
    /// Deployment-specific health I/O; HTTP formatting stays shared.
    async fn health(&self, request: &RequestHead) -> HttpResult<HealthCheck>;
}
