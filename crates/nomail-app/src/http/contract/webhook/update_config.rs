use super::endpoint;
use super::get_config::ConfigResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use crate::webhook::{
    PlatformConfig,
    config::{ConfigError, ConfigResult},
};
use serde::{Deserialize, Deserializer};

endpoint! {
    UpdateConfig {
        method: Patch,
        path: "/api/webhook/configs/{id}",
        auth: Bearer,
        operation_id: "updateWebhookConfig",
        tag: "Webhook Configs",
        summary: "修改接收配置",
        request: UpdateConfigRequest,
        query: (),
        response: ConfigResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: ConfigId,
    }
}

impl UpdateConfig {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<ConfigResponse> {
        Ok(call
            .services
            .configs
            .update(
                call.actor()?.user_id,
                call.matched.config_id()?,
                call.input::<Self>()?.try_into()?,
            )
            .await?
            .into())
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
impl TryFrom<UpdateConfigRequest> for crate::webhook::config::UpdateConfig {
    type Error = ConfigError;
    fn try_from(request: UpdateConfigRequest) -> ConfigResult<Self> {
        Self::new(request.name, request.platform, request.description)
    }
}
