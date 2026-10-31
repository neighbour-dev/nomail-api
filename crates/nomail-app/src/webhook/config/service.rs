use super::{ConfigError, ConfigRequest, ConfigResult, ConfigView};
use crate::{
    pagination::{Page, PaginationQuery},
    webhook::Provider,
};
use chrono::{DateTime, Utc};

/// No plaintext credentials cross the storage boundary. Deliberately not Debug/Serialize.
pub struct NewConfig {
    pub user_id: i64,
    pub name: String,
    pub key: String,
    pub provider: Provider,
    pub source: String,
    pub encrypted_secret: String,
    pub description: String,
    pub enabled: bool,
    pub at: DateTime<Utc>,
}
#[derive(Default)]
pub struct ConfigPatch {
    pub name: Option<String>,
    pub provider: Option<Provider>,
    pub source: Option<String>,
    pub encrypted_secret: Option<String>,
    pub description: Option<String>,
}
pub enum ConfigMutation {
    Patch(ConfigPatch),
    Enabled(bool),
    Delete,
}
pub struct ConfigPage {
    pub page: u64,
    pub size: u64,
    pub offset: u64,
}
#[allow(async_fn_in_trait)]
pub trait WebhookConfigStore {
    /// List and count must be a coherent read of undeleted, owned configs.
    async fn list_owned(
        &self,
        user_id: i64,
        page: &ConfigPage,
    ) -> ConfigResult<(Vec<ConfigView>, u64)>;
    async fn get_owned(&self, user_id: i64, id: i64) -> ConfigResult<Option<ConfigView>>;
    async fn insert(&self, config: &NewConfig) -> ConfigResult<ConfigView>;
    /// Atomically enforce ownership/visibility, write only selected columns and
    /// read the resulting projection. Delete returns the pre-deletion projection.
    async fn mutate_owned(
        &self,
        user_id: i64,
        id: i64,
        mutation: &ConfigMutation,
        at: DateTime<Utc>,
    ) -> ConfigResult<Option<ConfigView>>;
}
pub trait WebhookConfigRuntime {
    fn now(&self) -> DateTime<Utc>;
    fn random_bytes(&self, bytes: &mut [u8]) -> ConfigResult<()>;
    fn encryption_key(&self) -> &[u8; 32];
}
pub struct WebhookConfigService<S, R> {
    pub store: S,
    pub runtime: R,
}
fn id(value: &str) -> ConfigResult<i64> {
    value
        .parse()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| ConfigError::InvalidInput("invalid webhook config id".into()))
}
fn supplied(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
impl<S: WebhookConfigStore, R: WebhookConfigRuntime> WebhookConfigService<S, R> {
    fn encrypt(&self, secret: &str) -> ConfigResult<String> {
        if secret.is_empty() {
            return Ok(String::new());
        }
        let mut nonce = [0; 12];
        self.runtime.random_bytes(&mut nonce)?;
        crate::security::encrypt(secret, self.runtime.encryption_key(), &nonce)
            .map_err(|e| ConfigError::Internal(e.into()))
    }
    pub async fn list(
        &self,
        user_id: i64,
        query: PaginationQuery,
    ) -> ConfigResult<Page<ConfigView>> {
        let (page, size) =
            crate::pagination::pagination(query.page.as_deref(), query.page_size.as_deref());
        let offset = (page - 1)
            .checked_mul(size)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(|| ConfigError::InvalidInput("page is too large".into()))?;
        let (items, total) = self
            .store
            .list_owned(user_id, &ConfigPage { page, size, offset })
            .await?;
        Ok(Page {
            items,
            total,
            page,
            page_size: size,
        })
    }
    pub async fn get(&self, user_id: i64, value: &str) -> ConfigResult<ConfigView> {
        self.store
            .get_owned(user_id, id(value)?)
            .await?
            .ok_or(ConfigError::NotFound)
    }
    pub async fn create(&self, user_id: i64, body: ConfigRequest) -> ConfigResult<ConfigView> {
        body.validate(true).map_err(ConfigError::InvalidInput)?;
        let provider = super::super::service::configured_provider(&body.provider)
            .map_err(|e| ConfigError::InvalidInput(e.to_string()))?;
        let mut key = [0; 16];
        self.runtime.random_bytes(&mut key)?;
        let config = NewConfig {
            user_id,
            name: body.name,
            key: hex::encode(key),
            provider,
            source: body.source,
            encrypted_secret: self.encrypt(&body.secret)?,
            description: body.description,
            enabled: true,
            at: self.runtime.now(),
        };
        self.store.insert(&config).await
    }
    pub async fn update(
        &self,
        user_id: i64,
        value: &str,
        body: ConfigRequest,
    ) -> ConfigResult<ConfigView> {
        body.validate(false).map_err(ConfigError::InvalidInput)?;
        let id = id(value)?;
        // An empty PATCH is a read, including no change to updated_at.
        if body.name.is_empty()
            && body.provider.is_empty()
            && body.source.is_empty()
            && body.secret.is_empty()
            && body.description.is_empty()
        {
            return self
                .store
                .get_owned(user_id, id)
                .await?
                .ok_or(ConfigError::NotFound);
        }
        let provider = supplied(body.provider)
            .map(|p| {
                super::super::service::configured_provider(&p)
                    .map_err(|e| ConfigError::InvalidInput(e.to_string()))
            })
            .transpose()?;
        let encrypted_secret = supplied(body.secret)
            .map(|s| self.encrypt(&s))
            .transpose()?;
        let patch = ConfigPatch {
            name: supplied(body.name),
            provider,
            source: supplied(body.source),
            encrypted_secret,
            description: supplied(body.description),
        };
        self.mutate(user_id, id, ConfigMutation::Patch(patch)).await
    }
    async fn mutate(
        &self,
        user_id: i64,
        id: i64,
        mutation: ConfigMutation,
    ) -> ConfigResult<ConfigView> {
        self.store
            .mutate_owned(user_id, id, &mutation, self.runtime.now())
            .await?
            .ok_or(ConfigError::NotFound)
    }
    pub async fn enabled(
        &self,
        user_id: i64,
        value: &str,
        enabled: bool,
    ) -> ConfigResult<ConfigView> {
        self.mutate(user_id, id(value)?, ConfigMutation::Enabled(enabled))
            .await
    }
    pub async fn delete(&self, user_id: i64, value: &str) -> ConfigResult<()> {
        self.mutate(user_id, id(value)?, ConfigMutation::Delete)
            .await?;
        Ok(())
    }
}
