use super::{DEFAULT_RETENTION_DAYS, MessageAccepted, MessageError, MessageResult};
use crate::webhook::{
    EventMetadata, PlatformData, PlatformEvent, Provider, service::apply_inbox_rules,
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
    pub platform_data: PlatformData,
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
            raw_payload: event.raw.to_readable_string(),
            content_type: event.raw.media_type().into(),
            metadata: event.metadata,
            platform_data: event.data,
            received_at,
            updated_at: received_at,
            deleted_at: None,
            status: "unread".into(),
            pinned: false,
            expires_at: Some(expires_at),
            read_at: None,
        })
    }
}

/// A coherent detail read, not another business message or ORM entity.
pub struct MessageWithConfig {
    pub message: Message,
    pub config_name: String,
}
/// Raw response view; no need to load/decode platform JSON just to inspect raw.
pub struct RawContent {
    pub text: String,
    pub content_type: String,
}
/// Acceptance result/effect, not a duplicate message payload. Only a first
/// successful insert produces a best-effort online event.
pub struct Acceptance {
    pub outcome: InsertOutcome,
    pub created_event: Option<MessageAccepted>,
}
