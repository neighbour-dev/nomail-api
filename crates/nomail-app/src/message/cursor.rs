use super::{MessageFilters, MessagePreview, MessageQuery, MessageResult};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Stable position in receipt order, independent of the continued existence of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MessagePosition {
    pub received_at: DateTime<Utc>,
    pub id: Uuid,
}
impl From<&MessagePreview> for MessagePosition {
    fn from(message: &MessagePreview) -> Self {
        Self {
            received_at: message.received_at,
            id: message.id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageWindow {
    Latest,
    Before(MessagePosition),
    After(MessagePosition),
}
impl MessageWindow {
    pub fn ascending(self) -> bool {
        matches!(self, Self::After(_))
    }
}

pub struct MessageCursorQuery {
    // Reuse filter validation, without exposing offset pagination to storage.
    filters: MessageFilters,
    window: MessageWindow,
    limit: u64,
}
impl MessageCursorQuery {
    pub fn new(filters: MessageFilters, window: MessageWindow, limit: u64) -> MessageResult<Self> {
        let validated = MessageQuery::new(filters, 1, limit)?;
        Ok(Self {
            filters: validated.filters().clone(),
            window,
            limit,
        })
    }
    pub fn filters(&self) -> &MessageFilters {
        &self.filters
    }
    pub fn window(&self) -> MessageWindow {
        self.window
    }
    /// Read one extra row to detect continuation, without COUNT(*).
    pub fn fetch_limit(&self) -> u64 {
        self.limit + 1
    }
    pub fn into_page(self, mut items: Vec<MessagePreview>) -> MessageCursorPage {
        let has_more = items.len() > self.limit as usize;
        items.truncate(self.limit as usize);
        let start = items.first().map(MessagePosition::from);
        let end = items.last().map(MessagePosition::from);
        MessageCursorPage {
            items,
            has_more,
            next: if has_more { end } else { None },
            start,
            end,
        }
    }
}

pub struct MessageCursorPage {
    pub items: Vec<MessagePreview>,
    pub has_more: bool,
    pub next: Option<MessagePosition>,
    pub start: Option<MessagePosition>,
    pub end: Option<MessagePosition>,
}
