use super::{MessageError, MessagePreview, MessageResult};
use crate::pagination::PageSize;
use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};
use garde::Validate;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadStatus {
    Read,
    Unread,
}
impl ReadStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Unread => "unread",
        }
    }
}

#[derive(Clone, Debug, Default, Validate)]
pub struct MessageFilters {
    #[garde(skip)]
    pub text: Option<String>,
    #[garde(skip)]
    pub status: Option<ReadStatus>,
    #[garde(skip)]
    pub level: Option<MessageLevel>,
    #[garde(range(min = 1))]
    pub webhook_config_id: Option<i64>,
    #[garde(skip)]
    pub pinned: Option<bool>,
    #[garde(inner(custom(|from, _| validate_from(from, self.to))))]
    pub from: Option<DateTime<Utc>>,
    #[garde(skip)]
    pub to: Option<DateTime<Utc>>,
}

fn validate_from(from: &DateTime<Utc>, to: Option<DateTime<Utc>>) -> garde::Result {
    if to.is_some_and(|to| *from > to) {
        return Err(garde::Error::new("from must not be after to"));
    }
    Ok(())
}

impl MessageFilters {
    pub(super) fn checked(mut self) -> MessageResult<Self> {
        self.validate().map_err(|report| {
            let message = if garde::select!(report, webhook_config_id).next().is_some() {
                "invalid webhookConfigId"
            } else {
                "from must not be after to"
            };
            MessageError::InvalidInput(message.into())
        })?;
        self.text = self.text.and_then(|text| {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        });
        Ok(self)
    }
}

/// Validated, read-only query. Storage sees types, not HTTP strings or syntax.
#[derive(Clone, Debug)]
pub struct MessageQuery {
    filters: MessageFilters,
    page: u64,
    page_size: u64,
    offset: u64,
}
impl MessageQuery {
    pub fn new(filters: MessageFilters, page: u64, page_size: u64) -> MessageResult<Self> {
        let filters = filters.checked()?;
        let offset = crate::pagination::checked_offset(page, page_size)
            .map_err(|message| MessageError::InvalidInput(message.into()))?;
        Ok(Self {
            filters,
            page,
            page_size,
            offset,
        })
    }
    pub fn filters(&self) -> &MessageFilters {
        &self.filters
    }
    pub fn page(&self) -> u64 {
        self.page
    }
    pub fn page_size(&self) -> u64 {
        self.page_size
    }
    pub fn offset(&self) -> u64 {
        self.offset
    }
}
impl Default for MessageQuery {
    fn default() -> Self {
        Self {
            filters: MessageFilters::default(),
            page: 1,
            page_size: 20,
            offset: 0,
        }
    }
}

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
    filters: MessageFilters,
    window: MessageWindow,
    limit: u64,
}
impl MessageCursorQuery {
    pub fn new(filters: MessageFilters, window: MessageWindow, limit: u64) -> MessageResult<Self> {
        let filters = filters.checked()?;
        PageSize(limit)
            .check()
            .map_err(|message| MessageError::InvalidInput(message.into()))?;
        Ok(Self {
            filters,
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
