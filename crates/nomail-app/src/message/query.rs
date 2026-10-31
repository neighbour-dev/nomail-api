use super::{MessageError, MessageResult};
use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};

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
    pub fn parse(value: &str) -> MessageResult<Self> {
        match value {
            "read" => Ok(Self::Read),
            "unread" => Ok(Self::Unread),
            _ => Err(MessageError::InvalidInput(
                "status must be unread or read".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MessageFilters {
    pub text: Option<String>,
    pub status: Option<ReadStatus>,
    pub level: Option<MessageLevel>,
    pub webhook_config_id: Option<i64>,
    pub pinned: Option<bool>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
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
    pub fn new(mut filters: MessageFilters, page: u64, page_size: u64) -> MessageResult<Self> {
        if filters.webhook_config_id.is_some_and(|id| id <= 0) {
            return Err(MessageError::InvalidInput("invalid webhookConfigId".into()));
        }
        if filters
            .from
            .zip(filters.to)
            .is_some_and(|(from, to)| from > to)
        {
            return Err(MessageError::InvalidInput(
                "from must not be after to".into(),
            ));
        }
        let offset = crate::pagination::checked_offset(page, page_size)
            .map_err(|message| MessageError::InvalidInput(message.into()))?;
        filters.text = filters.text.and_then(|text| {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        });
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
