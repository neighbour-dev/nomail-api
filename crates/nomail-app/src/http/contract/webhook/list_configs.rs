use super::common::{Page, default_page, default_page_size};
use super::endpoint;
use super::get_config::ConfigResponse;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    ListConfigs {
        method: Get,
        path: "/api/webhook/configs",
        auth: Bearer,
        operation_id: "listWebhookConfigs",
        tag: "Webhook Configs",
        summary: "接收配置列表",
        request: (),
        query: PaginationQuery,
        response: Page<ConfigResponse>,
        body_kind: None,
        response_kind: Json,
        has_query: true,
        path_kind: None,
    }
}

impl ListConfigs {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<Page<ConfigResponse>> {
        Ok(call
            .services
            .configs
            .list(call.actor()?.user_id, call.query::<Self>()?.try_into()?)
            .await?
            .into())
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    #[cfg_attr(feature = "openapi", schemars(range(min = 1)))]
    pub page: u64,
    #[serde(default = "default_page_size")]
    #[cfg_attr(feature = "openapi", schemars(range(min = 1, max = crate::pagination::MAX_PAGE_SIZE)))]
    pub page_size: u64,
}

impl TryFrom<PaginationQuery> for crate::webhook::config::ConfigPage {
    type Error = crate::webhook::config::ConfigError;
    fn try_from(query: PaginationQuery) -> Result<Self, Self::Error> {
        Self::new(query.page, query.page_size)
    }
}
