use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use crate::webhook::{PlatformSummary, config::ConfigView};
use serde::{Deserialize, Serialize};

endpoint! {
    GetConfig {
        method: Get,
        path: "/api/webhook/configs/{id}",
        auth: Bearer,
        operation_id: "getWebhookConfig",
        tag: "Webhook Configs",
        summary: "配置详情",
        request: (),
        query: (),
        response: ConfigResponse,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: ConfigId,
    }
}

impl GetConfig {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<ConfigResponse> {
        Ok(call
            .services
            .configs
            .get(call.actor()?.user_id, call.matched.config_id()?)
            .await?
            .into())
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
            url: crate::http::webhook_url(&config.key),
            id: config.id,
            name: config.name,
            key: config.key,
            platform: config.platform,
            enabled: config.enabled,
            description: config.description,
        }
    }
}
