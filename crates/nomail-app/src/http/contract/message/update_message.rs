use super::endpoint;
use super::get_message::MessageDetail;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    UpdateMessage {
        method: Patch,
        path: "/api/messages/{id}",
        auth: Bearer,
        operation_id: "updateMessage",
        tag: "Messages",
        summary: "修改消息状态",
        request: UpdateMessageRequest,
        query: (),
        response: MessageDetail,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: MessageId,
    }
}

impl UpdateMessage {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<MessageDetail> {
        Ok(call
            .services
            .messages
            .update(
                call.actor()?.user_id,
                call.matched.message_id()?,
                call.input::<Self>()?.try_into()?,
                call.now(),
            )
            .await?
            .into())
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", schemars(rename = "UpdateRequest"))]
pub struct UpdateMessageRequest {
    pub status: Option<String>,
    pub pinned: Option<bool>,
}
impl TryFrom<UpdateMessageRequest> for crate::message::UpdateMessage {
    type Error = crate::message::MessageError;
    fn try_from(request: UpdateMessageRequest) -> Result<Self, Self::Error> {
        Self::new(
            request
                .status
                .as_deref()
                .map(super::message_filter::parse_read_status)
                .transpose()?,
            request.pinned,
        )
    }
}
