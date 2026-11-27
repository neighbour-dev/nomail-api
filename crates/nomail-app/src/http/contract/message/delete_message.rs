use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};

endpoint! {
    DeleteMessage {
        method: Delete,
        path: "/api/messages/{id}",
        auth: Bearer,
        operation_id: "deleteMessage",
        tag: "Messages",
        summary: "删除消息",
        request: (),
        query: (),
        response: (),
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: MessageId,
    }
}

impl DeleteMessage {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<()> {
        call.services
            .messages
            .delete(
                call.actor()?.user_id,
                call.matched.message_id()?,
                call.now(),
            )
            .await?;
        Ok(())
    }
}
