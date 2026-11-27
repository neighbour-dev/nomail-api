use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    DeleteConfig {
        method: Delete,
        path: "/api/webhook/configs/{id}",
        auth: Bearer,
        operation_id: "deleteWebhookConfig",
        tag: "Webhook Configs",
        summary: "删除接收配置",
        request: (),
        query: (),
        response: (),
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: ConfigId,
    }
}

impl DeleteConfig {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .configs
            .delete(call.actor()?.user_id, call.matched.config_id()?)
            .await?;
        Ok(())
    }
}
