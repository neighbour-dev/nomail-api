use super::endpoint;
use super::get_config::ConfigResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use crate::webhook::{
    PlatformConfig,
    config::{ConfigError, ConfigResult},
};
use serde::Deserialize;

endpoint! {
    CreateConfig {
        method: Post,
        path: "/api/webhook/configs",
        auth: Bearer,
        operation_id: "createWebhookConfig",
        tag: "Webhook Configs",
        summary: "创建接收配置",
        request: CreateConfigRequest,
        query: (),
        response: ConfigResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl CreateConfig {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<ConfigResponse> {
        Ok(call
            .services
            .configs
            .create(call.actor()?.user_id, call.input::<Self>()?.try_into()?)
            .await?
            .into())
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateConfigRequest {
    pub name: String,
    pub platform: PlatformConfig,
    #[serde(default)]
    pub description: String,
}
impl TryFrom<CreateConfigRequest> for crate::webhook::config::CreateConfig {
    type Error = ConfigError;
    fn try_from(request: CreateConfigRequest) -> ConfigResult<Self> {
        Self::new(request.name, request.platform, request.description)
    }
}
