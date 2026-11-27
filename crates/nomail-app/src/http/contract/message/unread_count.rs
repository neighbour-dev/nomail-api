use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    UnreadCount {
        method: Get,
        path: "/api/messages/unread-count",
        auth: Bearer,
        operation_id: "unreadCount",
        tag: "Messages",
        summary: "未读数量",
        request: (),
        query: (),
        response: CountResponse,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: None,
    }
}

impl UnreadCount {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<CountResponse> {
        Ok(CountResponse {
            count: call
                .services
                .messages
                .unread_count(call.actor()?.user_id)
                .await?,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct CountResponse {
    pub count: u64,
}
