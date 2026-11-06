//! Wire syntax is interpreted once, then use cases receive validated types.
use crate::message::{
    BatchMessages, MessageError, MessageFilters, MessageQuery, MessageResult, ReadStatus,
};
#[cfg(feature = "openapi")]
use crate::webhook::MessageLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", schemars(rename = "ListQuery"))]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesQuery {
    #[cfg_attr(feature = "openapi", schemars(with = "Option<u64>"))]
    pub page: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<u64>"))]
    pub page_size: Option<String>,
    #[serde(flatten)]
    pub filters: MessageFilterQuery,
}

#[derive(Default, Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageFilterQuery {
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
fn optional(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}
fn trimmed(value: Option<&str>) -> Option<&str> {
    optional(value.map(str::trim))
}
fn time(value: Option<&str>, name: &str) -> MessageResult<Option<DateTime<Utc>>> {
    optional(value)
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| MessageError::InvalidInput(format!("invalid {name}")))
        })
        .transpose()
}
impl TryFrom<MessageFilterQuery> for MessageFilters {
    type Error = MessageError;
    fn try_from(q: MessageFilterQuery) -> MessageResult<Self> {
        let status = trimmed(q.status.as_deref())
            .map(ReadStatus::parse)
            .transpose()?;
        let level = trimmed(q.level.as_deref())
            .map(|value| {
                value
                    .parse()
                    .map_err(|message: &str| MessageError::InvalidInput(message.into()))
            })
            .transpose()?;
        let webhook_config_id = optional(q.webhook_config_id.as_deref())
            .map(|value| {
                value
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id > 0)
                    .ok_or_else(|| MessageError::InvalidInput("invalid webhookConfigId".into()))
            })
            .transpose()?;
        let pinned = optional(q.pinned.as_deref())
            .map(|value| match value {
                "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
                "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
                _ => Err(MessageError::InvalidInput("invalid pinned".into())),
            })
            .transpose()?;
        let from = time(q.from.as_deref(), "from")?;
        let to = time(q.to.as_deref(), "to")?;
        Ok(MessageFilters {
            text: q.query,
            status,
            level,
            webhook_config_id,
            pinned,
            from,
            to,
        })
    }
}
impl TryFrom<ListMessagesQuery> for MessageQuery {
    type Error = MessageError;
    fn try_from(q: ListMessagesQuery) -> MessageResult<Self> {
        let filters = q.filters.try_into()?;
        let (page, size) = super::parse_pagination(q.page.as_deref(), q.page_size.as_deref());
        Self::new(filters, page, size)
    }
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", schemars(rename = "BatchRequest"))]
pub struct BatchMessagesRequest {
    pub ids: Vec<String>,
    pub action: String,
}
impl TryFrom<BatchMessagesRequest> for BatchMessages {
    type Error = MessageError;
    fn try_from(request: BatchMessagesRequest) -> MessageResult<Self> {
        Self::parse(request.ids, &request.action)
    }
}
