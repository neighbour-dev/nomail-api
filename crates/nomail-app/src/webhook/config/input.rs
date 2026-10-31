//! Validated business inputs. Plaintext credentials are deliberately not Debug/Serialize.
use super::{ConfigError, ConfigResult};
use crate::{
    auth::check_length,
    webhook::{Provider, service::configured_provider},
};

/// Positive configuration identity, independent of URL syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigId(i64);
impl ConfigId {
    pub fn new(value: i64) -> ConfigResult<Self> {
        if value <= 0 {
            return Err(ConfigError::InvalidInput(
                "invalid webhook config id".into(),
            ));
        }
        Ok(Self(value))
    }
    pub fn get(self) -> i64 {
        self.0
    }
}

pub struct CreateConfig {
    pub(super) name: String,
    pub(super) provider: Provider,
    pub(super) source: String,
    pub(super) secret: String,
    pub(super) description: String,
}
impl CreateConfig {
    pub fn new(
        name: String,
        provider: String,
        source: String,
        secret: String,
        description: String,
    ) -> ConfigResult<Self> {
        for (field, value, limit, required) in [
            ("name", name.as_str(), 100, true),
            ("provider", provider.as_str(), 32, true),
            ("source", source.as_str(), 255, false),
            ("secret", secret.as_str(), 500, false),
            ("description", description.as_str(), 500, false),
        ] {
            check_length(value, field, limit, required).map_err(ConfigError::InvalidInput)?;
        }
        let provider = configured_provider(&provider)
            .map_err(|error| ConfigError::InvalidInput(error.to_string()))?;
        Ok(Self {
            name,
            provider,
            source,
            secret,
            description,
        })
    }
}

#[derive(Default)]
pub struct UpdateConfig {
    pub(super) name: Option<String>,
    pub(super) provider: Option<Provider>,
    pub(super) source: Option<String>,
    pub(super) secret: Option<String>,
    pub(super) description: Option<String>,
}
impl UpdateConfig {
    pub fn new(
        name: Option<String>,
        provider: Option<String>,
        source: Option<String>,
        secret: Option<String>,
        description: Option<String>,
    ) -> ConfigResult<Self> {
        // Missing means keep; present means a nonempty, validated replacement.
        // Legacy empty-string handling belongs to the HTTP conversion.
        for (field, value, limit) in [
            ("name", name.as_deref(), 100),
            ("provider", provider.as_deref(), 32),
            ("source", source.as_deref(), 255),
            ("secret", secret.as_deref(), 500),
            ("description", description.as_deref(), 500),
        ] {
            if let Some(value) = value {
                check_length(value, field, limit, true).map_err(ConfigError::InvalidInput)?;
            }
        }
        let provider = provider
            .map(|value| {
                configured_provider(&value)
                    .map_err(|error| ConfigError::InvalidInput(error.to_string()))
            })
            .transpose()?;
        Ok(Self {
            name,
            provider,
            source,
            secret,
            description,
        })
    }
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.provider.is_none()
            && self.source.is_none()
            && self.secret.is_none()
            && self.description.is_none()
    }
}
