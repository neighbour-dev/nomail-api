use super::{Message, MessageQuery, MessageResult};
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
pub fn detail(m: &Message, config_id: i64, config_name: &str) -> MessageDetail {
    let content = &m.common;
    MessageDetail {
        id: m.id,
        title: content.title.clone(),
        content: content.content.clone(),
        source: content.source.clone().unwrap_or_default(),
        level: content.level,
        status: m.status.clone(),
        pinned: m.pinned,
        webhook_config: MessageConfigSummary {
            id: config_id,
            name: config_name.into(),
            provider: m.provider,
        },
        provider: m.provider,
        metadata: MessageMetadata {
            event_type: m.metadata.event_type.clone(),
            version: m.metadata.version.clone(),
            deduplication_key: m.metadata.deduplication_key.clone(),
        },
        platform_data: m.platform_data.clone(),
        created_at: m.received_at,
        updated_at: m.updated_at,
        expire_at: m.expires_at,
        read_at: m.read_at,
        url: content.url.clone().unwrap_or_default(),
        has_raw_payload: !m.raw_payload.is_empty(),
        raw_content_type: m.content_type.clone(),
    }
}

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[cfg_attr(feature = "openapi", schemars(with = "Option<u64>"))]
    pub page: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<u64>"))]
    pub page_size: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<MessageLevel>"))]
    pub level: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<i64>"))]
    pub webhook_config_id: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<bool>"))]
    pub pinned: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<DateTime<Utc>>"))]
    pub from: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<DateTime<Utc>>"))]
    pub to: Option<String>,
}
impl ListQuery {
    pub fn parsed(&self) -> MessageResult<MessageQuery> {
        MessageQuery::parse(self)
    }
    /// HTTP normalization delegates to the SAME typed query parser as the use
    /// case; it does not implement a second set of validation rules.
    pub fn normalize(&mut self) -> MessageResult<()> {
        let query = self.parsed()?;
        self.query = query.text;
        self.status = query.status.map(|s| s.as_str().into());
        self.level = query.level.map(|s| s.as_str().into());
        self.webhook_config_id = query.webhook_config_id.map(|id| id.to_string());
        self.pinned = query.pinned.map(|v| v.to_string());
        self.from = query.from.map(|v| v.to_rfc3339());
        self.to = query.to.map(|v| v.to_rfc3339());
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct UpdateRequest {
    pub status: Option<String>,
    pub pinned: Option<bool>,
}
impl UpdateRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.status.is_none() && self.pinned.is_none() {
            return Err("status or pinned is required");
        }
        if self
            .status
            .as_deref()
            .is_some_and(|s| !["read", "unread"].contains(&s))
        {
            return Err("status must be unread or read");
        }
        Ok(())
    }
}
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct BatchRequest {
    pub ids: Vec<String>,
    pub action: String,
}
