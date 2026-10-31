//! NoMail's custom Generic protocol. JSON fields and text header fields are inputs,
//! not defaults shared with other platforms.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum GenericEvent {
    Json(GenericJsonEvent),
    Text(GenericTextEvent),
}

/// Fields explicitly supported by the NoMail JSON protocol. Unknown fields stay in raw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GenericJsonEvent {
    pub title: Option<String>,
    pub content: Option<String>,
    pub message: Option<String>,
    pub body: Option<String>,
    pub source: Option<String>,
    pub level: Option<String>,
    pub url: Option<String>,
}

/// Inputs supplied through X-NoMail-* headers for a plain-text request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GenericTextEvent {
    pub title: Option<String>,
    pub source: Option<String>,
    pub level: Option<String>,
    pub url: Option<String>,
}

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
    let (raw, data, id, event_type) = match serde_json::from_slice::<Value>(request.body) {
        Ok(p) if p.is_object() || p.is_null() => {
            let fields = GenericJsonEvent {
                title: optional(text(&p, &["title"])),
                content: optional(text(&p, &["content"])),
                message: optional(text(&p, &["message"])),
                body: optional(text(&p, &["body"])),
                source: optional(text(&p, &["source"])),
                level: optional(text(&p, &["level"])),
                url: optional(text(&p, &["url"])),
            };
            let id = first(&[
                request.header("x-nomail-idempotency-key").into(),
                text(&p, &["id"]),
                text(&p, &["eventId"]),
            ]);
            let event_type = first(&[text(&p, &["eventType"]), text(&p, &["type"])]);
            (
                EventContent::Json(p),
                GenericEvent::Json(fields),
                id,
                event_type,
            )
        }
        _ => {
            let body = std::str::from_utf8(request.body)
                .map_err(|_| WebhookError::InvalidPayload("Generic text payload must be UTF-8"))?;
            if body.trim().is_empty() {
                return Err(WebhookError::InvalidPayload("empty payload"));
            }
            let fields = GenericTextEvent {
                title: optional(request.header("x-nomail-title")),
                source: optional(request.header("x-nomail-source")),
                level: optional(request.header("x-nomail-level")),
                url: optional(request.header("x-nomail-url")),
            };
            (
                EventContent::Text(body.to_owned()),
                GenericEvent::Text(fields),
                request.header("x-nomail-idempotency-key").into(),
                String::new(),
            )
        }
    };
    Ok(PlatformEvent {
        metadata: metadata(event_type, None, "generic:event", id),
        data: PlatformData::Generic(data),
        raw,
    }
    .into())
}

impl GenericEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        match self {
            Self::Text(fields) => message(
                fields
                    .title
                    .clone()
                    .unwrap_or_else(|| "Webhook notification".into()),
                raw.as_text().unwrap_or("").trim().to_owned(),
                fields.source.clone().unwrap_or_default(),
                fields.level.as_deref().unwrap_or(""),
                fields.url.clone().unwrap_or_default(),
            ),
            Self::Json(fields) => message(
                first(&[
                    fields.title.clone().unwrap_or_default(),
                    metadata.event_type.clone(),
                    "Webhook notification".into(),
                ]),
                first(&[
                    fields.content.clone().unwrap_or_default(),
                    fields.message.clone().unwrap_or_default(),
                    fields.body.clone().unwrap_or_default(),
                    raw.to_string(),
                ]),
                fields.source.clone().unwrap_or_default(),
                fields.level.as_deref().unwrap_or(""),
                fields.url.clone().unwrap_or_default(),
            ),
        }
    }
}
