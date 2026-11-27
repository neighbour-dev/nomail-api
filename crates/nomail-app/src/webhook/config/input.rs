//! Validated business inputs. Plaintext credentials are deliberately not Debug/Serialize.
use super::{ConfigError, ConfigResult};
use crate::{error::validate_input, webhook::PlatformConfig};
use garde::Validate;

const MAX_NAME_LENGTH: usize = 100;
const MAX_DESCRIPTION_LENGTH: usize = 500;

pub struct ConfigPage {
    page: u64,
    size: u64,
    offset: u64,
}

impl ConfigPage {
    pub fn new(page: u64, size: u64) -> ConfigResult<Self> {
        let offset = crate::pagination::checked_offset(page, size)
            .map_err(|message| ConfigError::InvalidInput(message.into()))?;
        Ok(Self { page, size, offset })
    }

    pub fn page(&self) -> u64 {
        self.page
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }
}

/// Positive configuration identity, independent of URL syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Validate)]
#[garde(transparent)]
pub struct ConfigId(#[garde(range(min = 1))] i64);
impl ConfigId {
    pub fn new(value: i64) -> ConfigResult<Self> {
        let id = Self(value);
        id.validate()
            .map_err(|_| ConfigError::InvalidInput("invalid webhook config id".into()))?;
        Ok(id)
    }
    pub fn get(self) -> i64 {
        self.0
    }
}

#[derive(Validate)]
pub struct CreateConfig {
    #[garde(length(chars, min = 1, max = MAX_NAME_LENGTH))]
    pub(super) name: String,
    #[garde(length(chars, max = MAX_DESCRIPTION_LENGTH))]
    pub(super) description: String,
    #[garde(custom(validate_platform))]
    pub(super) platform: PlatformConfig,
}
impl CreateConfig {
    pub fn new(name: String, platform: PlatformConfig, description: String) -> ConfigResult<Self> {
        let input = Self {
            name,
            platform,
            description,
        };
        validate_input(&input).map_err(ConfigError::InvalidInput)?;
        Ok(input)
    }
}

/// Missing fields are kept; a supplied platform is a complete replacement.
#[derive(Default, Validate)]
pub struct UpdateConfig {
    #[garde(length(chars, min = 1, max = MAX_NAME_LENGTH))]
    pub(super) name: Option<String>,
    #[garde(length(chars, max = MAX_DESCRIPTION_LENGTH))]
    pub(super) description: Option<String>,
    #[garde(inner(custom(validate_platform)))]
    pub(super) platform: Option<PlatformConfig>,
}
impl UpdateConfig {
    pub fn new(
        name: Option<String>,
        platform: Option<PlatformConfig>,
        description: Option<String>,
    ) -> ConfigResult<Self> {
        let input = Self {
            name,
            platform,
            description,
        };
        validate_input(&input).map_err(ConfigError::InvalidInput)?;
        Ok(input)
    }
    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.platform.is_none() && self.description.is_none()
    }
}

fn validate_platform(platform: &PlatformConfig, _: &()) -> garde::Result {
    platform
        .validate()
        .map_err(|error| garde::Error::new(error.to_string()))
}
