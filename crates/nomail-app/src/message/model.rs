use super::{DEFAULT_RETENTION_DAYS, MessageAccepted, MessageError, MessageResult};
use crate::webhook::{
    EventMetadata, PlatformData, PlatformEvent, Provider, RawHeader, RawWebhook, parse_raw,
    service::apply_inbox_rules,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use nomail_core::webhook::MessageContent;
/// Result of the single-record, atomic idempotent insert in either backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Created(Uuid),
    Duplicate(Uuid),
}
impl InsertOutcome {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Created(id) | Self::Duplicate(id) => *id,
        }
    }
    pub fn duplicate(&self) -> bool {
        matches!(self, Self::Duplicate(_))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub id: Uuid,
    pub webhook_config_id: i64,
    /// Reuse common content instead of another intermediate message DTO.
    #[serde(flatten)]
    pub common: MessageContent,
    pub provider: Provider,
    pub metadata: EventMetadata,
    pub raw_headers: Vec<RawHeader>,
    /// Only the decoded/decrypted payload, without a header/format/value wrapper.
    pub raw_payload: String,
    pub content_type: String,
    /// NoMail receipt time, not the upstream platform's occurrence time.
    #[serde(alias = "created_at")]
    pub received_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub status: String,
    pub pinned: bool,
    #[serde(alias = "expire_at")]
    pub expires_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
}

impl Message {
    /// Construct the complete inbox record without storage or runtime access.
    /// Receipt identity/time are trusted inputs, never taken from the platform.
    pub fn from_received_event(
        config_id: i64,
        event: PlatformEvent,
        id: Uuid,
        received_at: DateTime<Utc>,
    ) -> MessageResult<Self> {
        let provider = event.provider();
        let mut common = event.to_message_content();
        apply_inbox_rules(&mut common, provider);
        let expires_at = received_at
            .checked_add_signed(Duration::days(DEFAULT_RETENTION_DAYS))
            .ok_or_else(|| MessageError::Internal("invalid message expiry".into()))?;
        Ok(Self {
            id,
            webhook_config_id: config_id,
            common,
            provider,
            raw_payload: event.raw.stored_payload(),
            content_type: event.raw.payload_content_type().into(),
            raw_headers: event.raw.header,
            metadata: event.metadata,
            received_at,
            updated_at: received_at,
            deleted_at: None,
            status: "unread".into(),
            pinned: false,
            expires_at: Some(expires_at),
            read_at: None,
        })
    }

    pub fn raw(&self) -> MessageResult<RawWebhook> {
        RawWebhook::from_stored(
            self.raw_headers.clone(),
            &self.raw_payload,
            &self.content_type,
        )
        .map_err(|e| MessageError::Internal(e.to_string()))
    }

    pub fn platform_data(&self) -> MessageResult<PlatformData> {
        parse_raw(self.provider, &self.raw()?).map_err(|e| MessageError::Internal(e.to_string()))
    }
}

/// A coherent detail read, not another business message or ORM entity.
pub struct MessageWithConfig {
    pub message: Message,
    pub config_name: String,
}

/// Computed read projection, never a persisted platform snapshot.
pub struct MessageView {
    pub message: Message,
    pub config_name: String,
    pub platform_data: Option<PlatformData>,
    /// Safe diagnostic for unavailable projections; complete raw remains readable.
    pub platform_data_error: Option<String>,
}
impl MessageView {
    pub fn from_stored(row: MessageWithConfig) -> Self {
        let (platform_data, platform_data_error) = match row.message.platform_data() {
            Ok(data) => (Some(data), None),
            Err(error) => (None, Some(error.to_string())),
        };
        Self {
            message: row.message,
            config_name: row.config_name,
            platform_data,
            platform_data_error,
        }
    }
}

/// Raw-only storage view; no platform model or current credentials are loaded.
pub struct RawContent {
    /// JSON array contents as stored; do not project through typed header models.
    pub header: Vec<serde_json::Value>,
    pub text: String,
    pub content_type: String,
}
/// Acceptance result/effect, not a duplicate message payload. Only a first
/// successful insert produces a best-effort online event.
pub struct Acceptance {
    pub outcome: InsertOutcome,
    pub created_event: Option<MessageAccepted>,
}
