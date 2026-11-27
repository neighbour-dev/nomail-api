use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use serde::{Deserialize, Serialize};

endpoint! {
    RawMessage {
        method: Get,
        path: "/api/messages/{id}/raw",
        auth: Bearer,
        operation_id: "rawMessage",
        tag: "Messages",
        summary: "读取存储的请求头、原始 payload 字符串及 contentType",
        request: (),
        query: (),
        response: MessageRawResponse,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: MessageId,
    }
}

impl RawMessage {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<MessageRawResponse> {
        Ok(call
            .services
            .messages
            .raw(call.actor()?.user_id, call.matched.message_id()?)
            .await?
            .into())
    }
}

/// Stored raw data, without payload parsing or platform-model projection.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageRawResponse {
    /// Stored header entries, decoded only from their JSON storage representation.
    pub header: Vec<serde_json::Value>,
    /// Exact raw_payload string, including JSON whitespace or form encoding.
    pub payload: String,
    /// Stored payload serialization type, not the HTTP response Content-Type.
    pub content_type: String,
}
impl From<crate::message::RawContent> for MessageRawResponse {
    fn from(raw: crate::message::RawContent) -> Self {
        Self {
            header: raw.header,
            payload: raw.text,
            content_type: raw.content_type,
        }
    }
}
