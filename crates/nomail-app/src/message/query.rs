use super::{ListQuery, MessageError, MessageResult};
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
/// Validated business query, not another message model or raw HTTP map.
#[derive(Clone, Debug)]
pub struct MessageQuery {
    pub text: Option<String>,
    pub status: Option<ReadStatus>,
    pub level: Option<MessageLevel>,
    pub webhook_config_id: Option<i64>,
    pub pinned: Option<bool>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: u64,
    pub page_size: u64,
    pub offset: u64,
}
fn optional(value: Option<&str>) -> Option<&str> {
    value.filter(|s| !s.is_empty())
}
fn trimmed(value: Option<&str>) -> Option<&str> {
    optional(value.map(str::trim))
}
fn time(value: Option<&str>, name: &str) -> MessageResult<Option<DateTime<Utc>>> {
    optional(value)
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|v| v.with_timezone(&Utc))
                .map_err(|_| MessageError::InvalidInput(format!("invalid {name}")))
        })
        .transpose()
}
impl MessageQuery {
    pub fn parse(q: &ListQuery) -> MessageResult<Self> {
        let status = trimmed(q.status.as_deref())
            .map(ReadStatus::parse)
            .transpose()?;
        let level = trimmed(q.level.as_deref())
            .map(|s| {
                s.parse()
                    .map_err(|s: &str| MessageError::InvalidInput(s.into()))
            })
            .transpose()?;
        let webhook_config_id = optional(q.webhook_config_id.as_deref())
            .map(|s| {
                s.parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| MessageError::InvalidInput("invalid webhookConfigId".into()))
            })
            .transpose()?;
        let pinned = optional(q.pinned.as_deref())
            .map(|s| match s {
                "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
                "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
                _ => Err(MessageError::InvalidInput("invalid pinned".into())),
            })
            .transpose()?;
        let from = time(q.from.as_deref(), "from")?;
        let to = time(q.to.as_deref(), "to")?;
        if from.zip(to).is_some_and(|(a, b)| a > b) {
            return Err(MessageError::InvalidInput(
                "from must not be after to".into(),
            ));
        }
        let (page, page_size) =
            crate::pagination::pagination(q.page.as_deref(), q.page_size.as_deref());
        let offset = (page - 1)
            .checked_mul(page_size)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or_else(|| MessageError::InvalidInput("page is too large".into()))?;
        Ok(Self {
            text: trimmed(q.query.as_deref()).map(str::to_owned),
            status,
            level,
            webhook_config_id,
            pinned,
            from,
            to,
            page,
            page_size,
            offset,
        })
    }
}
