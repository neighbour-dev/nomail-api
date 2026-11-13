//! NoMail's custom Generic protocol. JSON fields and text header fields are inputs,
//! not defaults shared with other platforms.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] = &["x-nomail-signature"];
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

webhook_header!(GenericWebhookHeader {
    idempotency_key = "x-nomail-idempotency-key", title = "x-nomail-title",
    source = "x-nomail-source", level = "x-nomail-level", url = "x-nomail-url",
});
webhook!(GenericWebhook, GenericWebhookHeader, GenericWebhookPayload);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum GenericWebhookPayload {
    Json(Box<GenericJsonPayload>),
    Text(String),
    Null,
}

// Fields explicitly supported by the NoMail JSON protocol. Unknown fields stay in raw.
native_model!(GenericJsonPayload {
    title: String = "title",
    content: String = "content",
    message: String = "message",
    body: String = "body",
    source: String = "source",
    level: String = "level",
    url: String = "url",
    id: Value = "id",
    event_id: Value = "eventId",
    event_type: String = "eventType",
    type_name: String = "type",
});

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::HmacSha256(secret) if !secret.is_empty() => {
            crypto::hmac_header(secret, "x-nomail-signature", request)?
        }
        _ => return Err(invalid_auth()),
    }
    let (raw, id, event_type) = match serde_json::from_slice::<Value>(request.body) {
        Ok(p) if p.is_object() || p.is_null() => {
            let id = first(&[
                request.header("x-nomail-idempotency-key").into(),
                text(&p, &["id"]),
                text(&p, &["eventId"]),
            ]);
            let event_type = first(&[text(&p, &["eventType"]), text(&p, &["type"])]);
            (EventContent::Json(p), id, event_type)
        }
        _ => {
            let body = std::str::from_utf8(request.body)
                .map_err(|_| WebhookError::InvalidPayload("Generic text payload must be UTF-8"))?;
            if body.trim().is_empty() {
                return Err(WebhookError::InvalidPayload("empty payload"));
            }
            (
                EventContent::Text(body.to_owned()),
                request.header("x-nomail-idempotency-key").into(),
                String::new(),
            )
        }
    };
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(event_type, None, "generic:event", id),
        raw,
        request,
    )
    .into())
}

impl GenericWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        match &self.payload {
            GenericWebhookPayload::Text(_) => message(
                self.header
                    .title
                    .as_deref()
                    .and_then(optional)
                    .unwrap_or_else(|| "Webhook notification".into()),
                raw.as_text().unwrap_or("").trim().to_owned(),
                self.header.source.clone().unwrap_or_default(),
                self.header.level.as_deref().unwrap_or(""),
                self.header.url.clone().unwrap_or_default(),
            ),
            GenericWebhookPayload::Json(_) | GenericWebhookPayload::Null => {
                let p = raw.as_json().unwrap_or(&Value::Null);
                message(
                    first(&[
                        text(p, &["title"]),
                        metadata.event_type.clone(),
                        "Webhook notification".into(),
                    ]),
                    first(&[
                        text(p, &["content"]),
                        text(p, &["message"]),
                        text(p, &["body"]),
                        raw.to_string(),
                    ]),
                    text(p, &["source"]),
                    &text(p, &["level"]),
                    text(p, &["url"]),
                )
            }
        }
    }
}
