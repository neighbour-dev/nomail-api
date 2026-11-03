mod error;
mod input;
mod service;
pub use error::{ConfigError, ConfigResult};
pub use input::{ConfigId, CreateConfig, UpdateConfig};
pub use service::*;

#[cfg(test)]
mod tests;

/// Internal storage projection. Contains ciphertext, never an HTTP DTO.
pub struct ConfigRecord {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub provider: super::Provider,
    pub encrypted_platform: String,
    pub enabled: bool,
    pub description: String,
}
impl ConfigRecord {
    pub fn new(
        id: i64,
        name: String,
        key: String,
        provider: super::Provider,
        encrypted_platform: String,
        enabled: bool,
        description: String,
    ) -> Self {
        Self {
            id,
            name,
            key,
            provider,
            encrypted_platform,
            enabled,
            description,
        }
    }
}

pub struct ConfigView {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub platform: super::PlatformSummary,
    pub enabled: bool,
    pub description: String,
}

/// Decode only trusted encrypted storage. Never expose parse errors or plaintext.
pub(crate) fn decode_platform(
    ciphertext: &str,
    provider: super::Provider,
    key: &[u8; 32],
) -> Result<super::PlatformConfig, ()> {
    let plaintext = crate::security::decrypt(ciphertext, key).map_err(|_| ())?;
    let platform: super::PlatformConfig = serde_json::from_str(&plaintext).map_err(|_| ())?;
    platform.validate().map_err(|_| ())?;
    if platform.provider() != provider {
        return Err(());
    }
    Ok(platform)
}
