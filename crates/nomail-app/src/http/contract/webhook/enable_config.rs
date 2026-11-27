use super::endpoint;
use super::get_config::ConfigResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    EnableConfig {
        method: Patch,
        path: "/api/webhook/configs/{id}/enabled",
        auth: Bearer,
        operation_id: "enableWebhookConfig",
        tag: "Webhook Configs",
        summary: "启停接收配置",
        request: EnabledRequest,
        query: (),
        response: ConfigResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: ConfigId,
    }
}

impl EnableConfig {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<ConfigResponse> {
        Ok(call
            .services
            .configs
            .enabled(
                call.actor()?.user_id,
                call.matched.config_id()?,
                call.input::<Self>()?.enabled,
            )
            .await?
            .into())
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EnabledRequest {
    pub enabled: bool,
}
