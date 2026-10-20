use crate::{
    entity::{message, webhook},
    error::ApiError,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub page: Option<String>,
    pub page_size: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
    pub level: Option<String>,
    pub webhook_config_id: Option<String>,
    pub pinned: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

pub fn pagination(page: Option<&str>, size: Option<&str>) -> (u64, u64) {
    let page = page
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1) as u64;
    let size = size
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(20)
        .min(100) as u64;
    (page, size)
}
pub fn parse_time(value: Option<&str>, name: &str) -> Result<Option<DateTime<Utc>>, ApiError> {
    value
        .filter(|v| !v.is_empty())
        .map(|v| {
            DateTime::parse_from_rfc3339(v)
                .map(|v| v.with_timezone(&Utc))
                .map_err(|_| ApiError::bad(format!("invalid {name}")))
        })
        .transpose()
}
pub fn parse_bool(value: &str) -> Result<bool, ApiError> {
    match value {
        "1" | "t" | "T" | "TRUE" | "true" | "True" => Ok(true),
        "0" | "f" | "F" | "FALSE" | "false" | "False" => Ok(false),
        _ => Err(ApiError::bad("invalid pinned")),
    }
}
pub fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ApiError::bad("invalid message id"))
}
pub fn summarize(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect()
}

pub fn item(m: &message::Model) -> Value {
    json!({"id": m.id, "title": m.title, "summary": summarize(m.content.as_deref().unwrap_or("")),
        "source": m.source.as_deref().unwrap_or(""), "level": m.level, "status": m.status,
        "pinned": m.pinned.unwrap_or(false), "webhookConfigId": m.webhook_config_id,
        "createdAt": m.created_at, "expireAt": m.expire_at})
}
pub fn detail(m: &message::Model, config: &webhook::Model) -> Value {
    json!({"id": m.id, "title": m.title, "content": m.content.as_deref().unwrap_or(""),
        "source": m.source.as_deref().unwrap_or(""), "level": m.level, "status": m.status,
        "pinned": m.pinned.unwrap_or(false), "webhookConfig": {"id": config.id, "name": config.name, "provider": config.provider},
        "createdAt": m.created_at, "updatedAt": m.updated_at, "expireAt": m.expire_at,
        "readAt": m.read_at, "url": m.url.as_deref().unwrap_or(""),
        "hasRawPayload": m.raw_payload.as_ref().is_some_and(|p| !p.is_empty()),
        "rawContentType": m.content_type.as_deref().unwrap_or("")})
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub status: Option<String>,
    pub pinned: Option<bool>,
}
impl UpdateRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.status.is_none() && self.pinned.is_none() {
            return Err(ApiError::bad("status or pinned is required"));
        }
        if self
            .status
            .as_ref()
            .is_some_and(|s| !["read", "unread"].contains(&s.as_str()))
        {
            return Err(ApiError::bad("status must be unread or read"));
        }
        Ok(())
    }
}
#[derive(Deserialize)]
pub struct BatchRequest {
    pub ids: Vec<String>,
    pub action: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pagination_summary_and_validation() {
        assert_eq!(pagination(Some("0"), Some("999")), (1, 100));
        assert_eq!(pagination(Some("bad"), Some("-1")), (1, 20));
        assert_eq!(summarize("  hello\n世界  "), "hello 世界");
        assert_eq!(summarize(&"中".repeat(200)).chars().count(), 160);
        assert!(
            UpdateRequest {
                status: None,
                pinned: Some(false)
            }
            .validate()
            .is_ok()
        );
        assert!(
            UpdateRequest {
                status: Some("bad".into()),
                pinned: None
            }
            .validate()
            .is_err()
        );
        assert!(parse_time(Some("bad"), "from").is_err());
    }
}
