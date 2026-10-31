use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Lightweight storage projection; never loads platform JSON or raw payloads.
pub struct MessagePreview {
    pub id: Uuid,
    pub title: String,
    pub summary: String,
    pub source: Option<String>,
    pub level: MessageLevel,
    pub status: String,
    pub pinned: bool,
    pub webhook_config_id: i64,
    pub received_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}
