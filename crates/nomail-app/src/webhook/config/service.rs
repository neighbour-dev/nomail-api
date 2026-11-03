use super::{
    ConfigError, ConfigId, ConfigRecord, ConfigResult, ConfigView, CreateConfig, UpdateConfig,
};
use crate::{
    pagination::Page,
    webhook::{PlatformConfig, Provider},
};
use chrono::{DateTime, Utc};

/// No plaintext credentials cross the storage boundary. Deliberately not Debug/Serialize.
pub struct NewConfig {
    pub user_id: i64,
    pub name: String,
    pub key: String,
    pub provider: Provider,
    pub encrypted_platform: String,
    pub description: String,
    pub enabled: bool,
    pub at: DateTime<Utc>,
}
#[derive(Default)]
pub struct ConfigPatch {
    pub name: Option<String>,
    pub platform: Option<EncryptedPlatform>,
    pub description: Option<String>,
}
/// Provider and encrypted configuration must be replaced atomically.
pub struct EncryptedPlatform {
    pub provider: Provider,
    pub ciphertext: String,
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
        let offset = crate::pagination::checked_offset(page, size)
            .map_err(|message| ConfigError::InvalidInput(message.into()))?;
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
    ) -> ConfigResult<(Vec<ConfigRecord>, u64)>;
    async fn get_owned(&self, user_id: i64, id: i64) -> ConfigResult<Option<ConfigRecord>>;
    async fn insert(&self, config: &NewConfig) -> ConfigResult<ConfigRecord>;
    /// Atomically enforce ownership/visibility, write only selected columns and
    /// read the resulting projection. Delete returns the pre-deletion projection.
    async fn mutate_owned(
        &self,
        user_id: i64,
        id: i64,
        mutation: &ConfigMutation,
        at: DateTime<Utc>,
    ) -> ConfigResult<Option<ConfigRecord>>;
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
    fn encrypt(&self, platform: &PlatformConfig) -> ConfigResult<String> {
        let encoded = serde_json::to_string(platform)
            .map_err(|_| ConfigError::Internal("platform encoding failed".into()))?;
        let mut nonce = [0; 12];
        self.runtime.random_bytes(&mut nonce)?;
        crate::security::encrypt(&encoded, self.runtime.encryption_key(), &nonce)
            .map_err(|e| ConfigError::Internal(e.into()))
    }
    fn view(&self, record: ConfigRecord) -> ConfigResult<ConfigView> {
        let platform = super::decode_platform(
            &record.encrypted_platform,
            record.provider,
            self.runtime.encryption_key(),
        )
        .map_err(|_| ConfigError::Internal("invalid stored platform configuration".into()))?;
        Ok(ConfigView {
            id: record.id,
            name: record.name,
            key: record.key,
            platform: platform.summary(),
            enabled: record.enabled,
            description: record.description,
        })
    }
    pub async fn list(&self, user_id: i64, query: ConfigPage) -> ConfigResult<Page<ConfigView>> {
        let ConfigPage { page, size, .. } = query;
        let (items, total) = self.store.list_owned(user_id, &query).await?;
        Ok(Page {
            items: items
                .into_iter()
                .map(|record| self.view(record))
                .collect::<ConfigResult<_>>()?,
            total,
            page,
            page_size: size,
        })
    }
    pub async fn get(&self, user_id: i64, id: ConfigId) -> ConfigResult<ConfigView> {
        let record = self
            .store
            .get_owned(user_id, id.get())
            .await?
            .ok_or(ConfigError::NotFound)?;
        self.view(record)
    }
    pub async fn create(&self, user_id: i64, command: CreateConfig) -> ConfigResult<ConfigView> {
        let mut key = [0; 16];
        self.runtime.random_bytes(&mut key)?;
        let config = NewConfig {
            user_id,
            name: command.name,
            key: hex::encode(key),
            provider: command.platform.provider(),
            encrypted_platform: self.encrypt(&command.platform)?,
            description: command.description,
            enabled: true,
            at: self.runtime.now(),
        };
        self.view(self.store.insert(&config).await?)
    }
    pub async fn update(
        &self,
        user_id: i64,
        id: ConfigId,
        command: UpdateConfig,
    ) -> ConfigResult<ConfigView> {
        // An empty PATCH is a read, including no change to updated_at.
        if command.is_empty() {
            return self.get(user_id, id).await;
        }
        let platform = command
            .platform
            .map(|p| {
                Ok(EncryptedPlatform {
                    provider: p.provider(),
                    ciphertext: self.encrypt(&p)?,
                })
            })
            .transpose()?;
        let patch = ConfigPatch {
            name: command.name,
            platform,
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
        let record = self
            .store
            .mutate_owned(user_id, id.get(), &mutation, self.runtime.now())
            .await?
            .ok_or(ConfigError::NotFound)?;
        self.view(record)
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
        self.store
            .mutate_owned(
                user_id,
                id.get(),
                &ConfigMutation::Delete,
                self.runtime.now(),
            )
            .await?
            .ok_or(ConfigError::NotFound)?;
        Ok(())
    }
}
