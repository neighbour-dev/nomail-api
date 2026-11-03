//! Validated business inputs. Plaintext credentials are deliberately not Debug/Serialize.
use super::{ConfigError, ConfigResult};
use crate::{auth::check_length, webhook::PlatformConfig};

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
    pub(super) platform: PlatformConfig,
    pub(super) description: String,
}
impl CreateConfig {
    pub fn new(name: String, platform: PlatformConfig, description: String) -> ConfigResult<Self> {
        for (field, value, limit, required) in [
            ("name", name.as_str(), 100, true),
            ("description", description.as_str(), 500, false),
        ] {
            check_length(value, field, limit, required).map_err(ConfigError::InvalidInput)?;
        }
        platform
            .validate()
            .map_err(|error| ConfigError::InvalidInput(error.to_string()))?;
        Ok(Self {
            name,
            platform,
            description,
        })
    }
}

#[derive(Default)]
pub struct UpdateConfig {
    pub(super) name: Option<String>,
    pub(super) platform: Option<PlatformConfig>,
    pub(super) description: Option<String>,
}
impl UpdateConfig {
    pub fn new(
        name: Option<String>,
        platform: Option<PlatformConfig>,
        description: Option<String>,
    ) -> ConfigResult<Self> {
        // Missing means keep; platform is always a complete replacement.
        for (field, value, limit) in [
            ("name", name.as_deref(), 100),
            ("description", description.as_deref(), 500),
        ] {
            if let Some(value) = value {
                check_length(value, field, limit, field == "name")
                    .map_err(ConfigError::InvalidInput)?;
            }
        }
        if let Some(value) = &platform {
            value
                .validate()
                .map_err(|error| ConfigError::InvalidInput(error.to_string()))?;
        }
        Ok(Self {
            name,
            platform,
            description,
        })
    }
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.platform.is_none() && self.description.is_none()
    }
}
