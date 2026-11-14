//! Small shared data helpers. No platform routing, authentication policies or mappings.
use super::*;
use serde_json::{Map, Value, json};

pub(super) fn invalid_auth() -> WebhookError {
    WebhookError::InvalidConfiguration("invalid authentication configuration")
}

pub(super) fn text(value: &Value, path: &[&str]) -> String {
    let mut value = value;
    for key in path {
        value = &value[*key];
    }
    match value {
        Value::String(value) => value.trim().to_owned(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

pub(super) fn optional(value: impl Into<String>) -> Option<String> {
    let value = value.into().trim().to_owned();
    (!value.is_empty()).then_some(value)
}

pub(super) fn first(values: &[String]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default()
}

pub(super) fn object(body: &[u8]) -> Result<Value, WebhookError> {
    let value = json_payload(body)?;
    if !value.is_object() {
        return Err(WebhookError::InvalidPayload("expected a JSON object"));
    }
    Ok(value)
}

pub(super) fn json_payload(body: &[u8]) -> Result<Value, WebhookError> {
    serde_json::from_slice(body).map_err(|_| WebhookError::InvalidPayload("invalid JSON payload"))
}

pub(super) fn form(body: &[u8]) -> Result<Map<String, Value>, WebhookError> {
    let fields: Vec<(String, String)> = serde_urlencoded::from_bytes(body)
        .map_err(|_| WebhookError::InvalidPayload("invalid form payload"))?;
    let mut result = Map::new();
    for (key, value) in fields {
        if result.insert(key, Value::String(value)).is_some() {
            return Err(WebhookError::InvalidPayload("duplicate form field"));
        }
    }
    Ok(result)
}

pub(super) fn is_form(request: &WebhookRequest<'_>) -> bool {
    request
        .header("content-type")
        .split(';')
        .next()
        .is_some_and(|value| {
            value
                .trim()
                .eq_ignore_ascii_case("application/x-www-form-urlencoded")
        })
}

pub(super) fn required(value: String, message: &'static str) -> Result<String, WebhookError> {
    optional(value).ok_or(WebhookError::InvalidPayload(message))
}

pub(super) fn metadata(
    event_type: String,
    version: Option<String>,
    scope: &str,
    id: String,
) -> EventMetadata {
    EventMetadata {
        event_type,
        version,
        deduplication_key: optional(id).map(|value| DeduplicationKey {
            scope: scope.into(),
            value,
        }),
    }
}

pub(super) fn challenge(payload: &Value) -> Result<ReceiveOutcome, WebhookError> {
    let value = payload["challenge"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or(WebhookError::InvalidPayload("missing challenge"))?;
    Ok(ReceiveOutcome::Respond(ProtocolResponse {
        status: 200,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&json!({"challenge": value}))
            .map_err(|_| WebhookError::InvalidPayload("invalid challenge"))?,
    }))
}

pub(super) fn message(title: String, summary: String, level: &str, url: String) -> MessageContent {
    MessageContent {
        title,
        summary,
        level: MessageLevel::from_label(level),
        url: optional(url),
    }
}

/// Only declared platform fields may supply a preview; never serialize raw as
/// a fallback or coerce non-string JSON values into display text.
pub(super) fn summary(values: &[Option<&str>]) -> String {
    values
        .iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
        .map(|value| super::summarize(value))
        .unwrap_or_default()
}
