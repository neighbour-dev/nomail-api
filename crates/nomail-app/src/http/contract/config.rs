//! Product wire envelope around Core's strictly typed platform protocol configuration.
use crate::webhook::{
    PlatformConfig, PlatformSummary,
    config::{ConfigError, ConfigResult, ConfigView, CreateConfig, UpdateConfig},
};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateConfigRequest {
    pub name: String,
    pub platform: PlatformConfig,
    #[serde(default)]
    pub description: String,
}
impl TryFrom<CreateConfigRequest> for CreateConfig {
    type Error = ConfigError;
    fn try_from(request: CreateConfigRequest) -> ConfigResult<Self> {
        Self::new(request.name, request.platform, request.description)
    }
}

// Missing means keep; explicit null is not a valid replacement.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[cfg(feature = "openapi")]
fn omit_missing_default(schema: &mut schemars::Schema) {
    // Rust's Option::None means an omitted PATCH field, not a JSON null default.
    schema.remove("default");
}

#[derive(Default, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateConfigRequest {
    #[serde(default, deserialize_with = "present")]
    #[cfg_attr(feature = "openapi", schemars(with = "String", transform = omit_missing_default))]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[cfg_attr(feature = "openapi", schemars(with = "PlatformConfig", transform = omit_missing_default))]
    pub platform: Option<PlatformConfig>,
    #[serde(default, deserialize_with = "present")]
    #[cfg_attr(feature = "openapi", schemars(with = "String", transform = omit_missing_default))]
    pub description: Option<String>,
}
impl TryFrom<UpdateConfigRequest> for UpdateConfig {
    type Error = ConfigError;
    fn try_from(request: UpdateConfigRequest) -> ConfigResult<Self> {
        Self::new(request.name, request.platform, request.description)
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub platform: PlatformSummary,
    pub enabled: bool,
    pub description: String,
    pub url: String,
}
impl From<ConfigView> for ConfigResponse {
    fn from(config: ConfigView) -> Self {
        Self {
            url: super::super::webhook_url(&config.key),
            id: config.id,
            name: config.name,
            key: config.key,
            platform: config.platform,
            enabled: config.enabled,
            description: config.description,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EnabledRequest {
    pub enabled: bool,
}
