use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use crate::message::{BatchMessages as BatchCommand, MessageError, MessageResult};
use serde::{Deserialize, Serialize};

endpoint! {
    BatchMessages {
        method: Post,
        path: "/api/messages/batch",
        auth: Bearer,
        operation_id: "batchMessages",
        tag: "Messages",
        summary: "批量修改消息",
        request: BatchMessagesRequest,
        query: (),
        response: AffectedResponse,
        body_kind: Json,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl BatchMessages {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<AffectedResponse> {
        Ok(AffectedResponse {
            affected: call
                .services
                .messages
                .batch(
                    call.actor()?.user_id,
                    call.input::<Self>()?.try_into()?,
                    call.now(),
                )
                .await?,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", schemars(rename = "BatchRequest"))]
pub struct BatchMessagesRequest {
    pub ids: Vec<String>,
    pub action: String,
}
impl TryFrom<BatchMessagesRequest> for BatchCommand {
    type Error = MessageError;
    fn try_from(request: BatchMessagesRequest) -> MessageResult<Self> {
        Self::parse(request.ids, &request.action)
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AffectedResponse {
    pub affected: u64,
}
