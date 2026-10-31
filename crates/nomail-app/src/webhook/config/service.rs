use super::{ConfigError, ConfigId, ConfigResult, ConfigView, CreateConfig, UpdateConfig};
use crate::{pagination::Page, webhook::Provider};
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
    page: u64,
    size: u64,
    offset: u64,
}
impl ConfigPage {
    pub fn page(&self) -> u64 {
        self.page
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn offset(&self) -> u64 {
        self.offset
    }
    pub fn new(page: u64, size: u64) -> ConfigResult<Self> {
        if page == 0 || !(1..=100).contains(&size) {
            return Err(ConfigError::InvalidInput("invalid pagination".into()));
        }
        let offset = (page - 1)
            .checked_mul(size)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(|| ConfigError::InvalidInput("page is too large".into()))?;
        Ok(Self { page, size, offset })
    }
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
    pub async fn list(&self, user_id: i64, query: ConfigPage) -> ConfigResult<Page<ConfigView>> {
        let ConfigPage { page, size, .. } = query;
        let (items, total) = self.store.list_owned(user_id, &query).await?;
        Ok(Page {
            items,
            total,
            page,
            page_size: size,
        })
    }
    pub async fn get(&self, user_id: i64, id: ConfigId) -> ConfigResult<ConfigView> {
        self.store
            .get_owned(user_id, id.get())
            .await?
            .ok_or(ConfigError::NotFound)
    }
    pub async fn create(&self, user_id: i64, command: CreateConfig) -> ConfigResult<ConfigView> {
        let mut key = [0; 16];
        self.runtime.random_bytes(&mut key)?;
        let config = NewConfig {
            user_id,
            name: command.name,
            key: hex::encode(key),
            provider: command.provider,
            source: command.source,
            encrypted_secret: self.encrypt(&command.secret)?,
            description: command.description,
            enabled: true,
            at: self.runtime.now(),
        };
        self.store.insert(&config).await
    }
    pub async fn update(
        &self,
        user_id: i64,
        id: ConfigId,
        command: UpdateConfig,
    ) -> ConfigResult<ConfigView> {
        // An empty PATCH is a read, including no change to updated_at.
        if command.is_empty() {
            return self
                .store
                .get_owned(user_id, id.get())
                .await?
                .ok_or(ConfigError::NotFound);
        }
        let encrypted_secret = command.secret.map(|s| self.encrypt(&s)).transpose()?;
        let patch = ConfigPatch {
            name: command.name,
            provider: command.provider,
            source: command.source,
            encrypted_secret,
            description: command.description,
        };
        self.mutate(user_id, id, ConfigMutation::Patch(patch)).await
    }
    async fn mutate(
        &self,
        user_id: i64,
        id: ConfigId,
        mutation: ConfigMutation,
    ) -> ConfigResult<ConfigView> {
        self.store
            .mutate_owned(user_id, id.get(), &mutation, self.runtime.now())
            .await?
            .ok_or(ConfigError::NotFound)
    }
    pub async fn enabled(
        &self,
        user_id: i64,
        id: ConfigId,
        enabled: bool,
    ) -> ConfigResult<ConfigView> {
        self.mutate(user_id, id, ConfigMutation::Enabled(enabled))
            .await
    }
    pub async fn delete(&self, user_id: i64, id: ConfigId) -> ConfigResult<()> {
        self.mutate(user_id, id, ConfigMutation::Delete).await?;
        Ok(())
    }
}
