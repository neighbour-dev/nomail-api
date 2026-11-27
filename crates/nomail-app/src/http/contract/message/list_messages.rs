use super::common::{Page, default_page, default_page_size};
use super::endpoint;
use super::message_filter::MessageFilterQuery;
use crate::http::{Environment, HttpResult, dispatch::EndpointCall};
use crate::message::{MessageError, MessagePreview, MessageQuery, MessageResult};
use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

endpoint! {
    ListMessages {
        method: Get,
        path: "/api/messages",
        auth: Bearer,
        operation_id: "listMessages",
        tag: "Messages",
        summary: "消息列表",
        request: (),
        query: ListMessagesQuery,
        response: Page<MessageListItem>,
        body_kind: None,
        response_kind: Json,
        has_query: true,
        path_kind: None,
    }
}

impl ListMessages {
    pub(in crate::http) async fn handle(
        call: EndpointCall<'_, impl Environment>,
    ) -> HttpResult<Page<MessageListItem>> {
        Ok(call
            .services
            .messages
            .list(call.actor()?.user_id, call.query::<Self>()?.try_into()?)
            .await?
            .into())
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", schemars(rename = "ListQuery"))]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesQuery {
    #[serde(default = "default_page")]
    #[cfg_attr(feature = "openapi", schemars(range(min = 1)))]
    pub page: u64,
    #[serde(default = "default_page_size")]
    #[cfg_attr(feature = "openapi", schemars(range(min = 1, max = crate::pagination::MAX_PAGE_SIZE)))]
    pub page_size: u64,
    #[serde(flatten)]
    pub filters: MessageFilterQuery,
}

impl TryFrom<ListMessagesQuery> for MessageQuery {
    type Error = MessageError;
    fn try_from(q: ListMessagesQuery) -> MessageResult<Self> {
        let filters = q.filters.try_into()?;
        Self::new(filters, q.page, q.page_size)
    }
}

/// HTTP list projection only, not another business message. Repositories select
/// these columns without loading raw headers/payloads or parsing platform data.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageListItem {
    pub id: Uuid,
    pub title: String,
    /// Persisted receipt-time preview; legacy rows may have an empty string.
    #[cfg_attr(feature = "openapi", schemars(length(max = 30)))]
    pub summary: String,
    pub level: MessageLevel,
    pub status: String,
    pub pinned: bool,
    #[serde(alias = "webhook_config_id")]
    pub webhook_config_id: i64,
    #[serde(rename = "createdAt", alias = "created_at")]
    pub received_at: DateTime<Utc>,
    #[serde(rename = "expireAt", alias = "expire_at")]
    pub expires_at: Option<DateTime<Utc>>,
}

impl From<MessagePreview> for MessageListItem {
    fn from(m: MessagePreview) -> Self {
        Self {
            id: m.id,
            title: m.title,
            summary: m.summary,
            level: m.level,
            status: m.status,
            pinned: m.pinned,
            webhook_config_id: m.webhook_config_id,
            received_at: m.received_at,
            expires_at: m.expires_at,
        }
    }
}
