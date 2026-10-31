use crate::auth::check_length;
use serde::{Deserialize, Serialize};

mod error;
mod service;
pub use error::{ConfigError, ConfigResult};
pub use service::*;

#[cfg(test)]
mod tests;

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConfigRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub description: String,
}
impl ConfigRequest {
    pub const REQUIRED_CREATE_FIELDS: &'static [&'static str] = &["name", "provider"];
    pub fn validate(&self, required: bool) -> Result<(), String> {
        check_length(&self.name, "name", 100, required)?;
        check_length(&self.provider, "provider", 32, required)?;
        check_length(&self.source, "source", 255, false)?;
        check_length(&self.secret, "secret", 500, false)?;
        check_length(&self.description, "description", 500, false)?;
        if required || !self.provider.is_empty() {
            super::service::configured_provider(&self.provider).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct EnabledRequest {
    pub enabled: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConfigView {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub provider: super::Provider,
    pub source: String,
    pub enabled: bool,
    pub description: String,
}
impl ConfigView {
    pub fn new(
        id: i64,
        name: String,
        key: String,
        provider: super::Provider,
        source: String,
        enabled: bool,
        description: String,
    ) -> Self {
        Self {
            id,
            name,
            key,
            provider,
            source,
            enabled,
            description,
        }
    }
}
