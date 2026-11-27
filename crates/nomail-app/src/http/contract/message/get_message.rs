use super::endpoint;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use crate::message::MessageView;
use crate::webhook::{MessageLevel, PlatformData, Provider};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

endpoint! {
    GetMessage {
        method: Get,
        path: "/api/messages/{id}",
        auth: Bearer,
        operation_id: "getMessage",
        tag: "Messages",
        summary: "消息详情",
        request: (),
        query: (),
        response: MessageDetail,
        body_kind: None,
        response_kind: Json,
        has_query: false,
        path_kind: MessageId,
    }
}

impl GetMessage {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<MessageDetail> {
        Ok(call
            .services
            .messages
            .get(call.actor()?.user_id, call.matched.message_id()?)
            .await?
            .into())
    }
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageConfigSummary {
    pub id: i64,
    pub name: String,
    pub provider: Provider,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageMetadata {
    pub event_type: String,
    pub version: Option<String>,
    pub deduplication_key: Option<crate::webhook::DeduplicationKey>,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    pub id: Uuid,
    pub title: String,
    pub level: MessageLevel,
    pub status: String,
    pub pinned: bool,
    pub webhook_config: MessageConfigSummary,
    pub provider: Provider,
    pub metadata: MessageMetadata,
    /// Computed from decoded raw on read; never a database platform snapshot.
    pub platform_data: Option<PlatformData>,
    /// Present when platformData is unavailable; raw and message state remain usable.
    pub platform_data_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub url: String,
    pub has_raw_payload: bool,
    pub raw_content_type: String,
}
impl From<MessageView> for MessageDetail {
    fn from(row: MessageView) -> Self {
        let MessageView {
            message: m,
            config_name,
            platform_data,
            platform_data_error,
        } = row;
        let content = m.common;
        MessageDetail {
            id: m.id,
            title: content.title,
            level: content.level,
            status: m.status,
            pinned: m.pinned,
            webhook_config: MessageConfigSummary {
                id: m.webhook_config_id,
                name: config_name,
                provider: m.provider,
            },
            provider: m.provider,
            metadata: MessageMetadata {
                event_type: m.metadata.event_type,
                version: m.metadata.version,
                deduplication_key: m.metadata.deduplication_key,
            },
            platform_data,
            platform_data_error,
            created_at: m.received_at,
            updated_at: m.updated_at,
            expire_at: m.expires_at,
            read_at: m.read_at,
            url: content.url.unwrap_or_default(),
            // Raw storage is mandatory; an empty decoded form/text is still raw.
            has_raw_payload: true,
            raw_content_type: m.content_type,
        }
    }
}
