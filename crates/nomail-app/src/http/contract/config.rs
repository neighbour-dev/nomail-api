//! Configuration wire inputs and presentation. Use cases receive validated commands.
use crate::webhook::{
    Provider,
    config::{ConfigError, ConfigResult, ConfigView, CreateConfig, UpdateConfig},
};
use serde::{Deserialize, Serialize};

// Required fields plus flattened options retain the object-only creation body
// without a second JSON parse. All string fields remain strict about null/types.
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateConfigRequest {
    pub name: String,
    pub provider: String,
    #[serde(flatten)]
    options: CreateConfigOptions,
}
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
struct CreateConfigOptions {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub description: String,
}
impl TryFrom<CreateConfigRequest> for CreateConfig {
    type Error = ConfigError;
    fn try_from(request: CreateConfigRequest) -> ConfigResult<Self> {
        Self::new(
            request.name,
            request.provider,
            request.options.source,
            request.options.secret,
            request.options.description,
        )
    }
}

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfigRequest {
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
fn supplied(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
impl TryFrom<UpdateConfigRequest> for UpdateConfig {
    type Error = ConfigError;
    fn try_from(request: UpdateConfigRequest) -> ConfigResult<Self> {
        Self::new(
            supplied(request.name),
            supplied(request.provider),
            supplied(request.source),
            supplied(request.secret),
            supplied(request.description),
        )
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub provider: Provider,
    pub source: String,
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
            provider: config.provider,
            source: config.source,
            enabled: config.enabled,
            description: config.description,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct EnabledRequest {
    pub enabled: bool,
}
