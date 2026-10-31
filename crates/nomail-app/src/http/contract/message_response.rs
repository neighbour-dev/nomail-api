use crate::message::{MessagePreview, MessageWithConfig};
use crate::webhook::{MessageLevel, PlatformData, Provider};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// HTTP list projection only, not another business message. Repositories select
/// these columns without loading platform JSON or raw payloads for each row.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageListItem {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub source: String,
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
    pub content: String,
    pub source: String,
    pub level: MessageLevel,
    pub status: String,
    pub pinned: bool,
    pub webhook_config: MessageConfigSummary,
    pub provider: Provider,
    pub metadata: MessageMetadata,
    pub platform_data: PlatformData,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub url: String,
    pub has_raw_payload: bool,
    pub raw_content_type: String,
}
impl From<MessageWithConfig> for MessageDetail {
    fn from(row: MessageWithConfig) -> Self {
        let MessageWithConfig {
            message: m,
            config_name,
        } = row;
        let content = m.common;
        MessageDetail {
            id: m.id,
            title: content.title,
            content: content.content,
            source: content.source.unwrap_or_default(),
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
            platform_data: m.platform_data,
            created_at: m.received_at,
            updated_at: m.updated_at,
            expire_at: m.expires_at,
            read_at: m.read_at,
            url: content.url.unwrap_or_default(),
            has_raw_payload: !m.raw_payload.is_empty(),
            raw_content_type: m.content_type,
        }
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
                .map(crate::message::ReadStatus::parse)
                .transpose()?,
            request.pinned,
        )
    }
}

impl From<MessagePreview> for MessageListItem {
    fn from(m: MessagePreview) -> Self {
        Self {
            id: m.id,
            title: m.title,
            summary: m.summary,
            source: m.source.unwrap_or_default(),
            level: m.level,
            status: m.status,
            pinned: m.pinned,
            webhook_config_id: m.webhook_config_id,
            received_at: m.received_at,
            expires_at: m.expires_at,
        }
    }
}
