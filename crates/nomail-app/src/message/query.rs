use super::{MessageError, MessageResult};
use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};
use garde::Validate;

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
        filters.validate().map_err(|report| {
            let message = if garde::select!(report, webhook_config_id).next().is_some() {
                "invalid webhookConfigId"
            } else {
                "from must not be after to"
            };
            MessageError::InvalidInput(message.into())
        })?;
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
