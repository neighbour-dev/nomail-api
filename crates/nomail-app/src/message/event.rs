//! Business event emitted after the first durable acceptance, not an SSE frame.
use super::Message;
use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct MessageAccepted {
    pub id: Uuid,
    pub title: String,
    pub source: String,
    pub level: MessageLevel,
    pub status: String,
    pub pinned: bool,
    pub webhook_config_id: i64,
    pub received_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}
impl From<&Message> for MessageAccepted {
    fn from(message: &Message) -> Self {
        Self {
            id: message.id,
            title: message.common.title.clone(),
            source: message.common.source.clone().unwrap_or_default(),
            level: message.common.level,
            status: message.status.clone(),
            pinned: message.pinned,
            webhook_config_id: message.webhook_config_id,
            received_at: message.received_at,
            expires_at: message.expires_at,
        }
    }
}
