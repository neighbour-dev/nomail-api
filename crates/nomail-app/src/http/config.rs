//! HTTP presentation of a credential-free integration view.
use crate::{
    pagination::Page,
    webhook::{Provider, config::ConfigView},
};
use serde::{Deserialize, Serialize};

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
            url: super::webhook_url(&config.key),
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
impl From<Page<ConfigView>> for Page<ConfigResponse> {
    fn from(page: Page<ConfigView>) -> Self {
        page.map(ConfigResponse::from)
    }
}
