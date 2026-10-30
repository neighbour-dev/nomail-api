//! Generic webhook authentication, payload model and built-in inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventMetadata, IncomingMessage, PlatformEvent, ReceiveOutcome, WebhookConfig,
    WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenericEvent {
    pub metadata: EventMetadata,
    pub payload: GenericPayload,
    pub defaults: GenericDefaults,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenericDefaults {
    pub title: String,
    pub source: String,
    pub level: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenericPayload {
    Json(Value),
    Text(String),
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
    let payload = match serde_json::from_slice::<Value>(request.body) {
        Ok(value) if value.is_object() || value.is_null() => GenericPayload::Json(value),
        _ => {
            let text = String::from_utf8_lossy(request.body).trim().to_owned();
            if text.is_empty() {
                return Err(WebhookError::InvalidPayload("empty payload"));
            }
            GenericPayload::Text(text)
        }
    };
    let (id, event_type) = match &payload {
        GenericPayload::Json(value) => (
            first(&[
                request.header("x-nomail-idempotency-key").into(),
                text(value, &["id"]),
                text(value, &["eventId"]),
            ]),
            first(&[text(value, &["eventType"]), text(value, &["type"])]),
        ),
        GenericPayload::Text(_) => (
            request.header("x-nomail-idempotency-key").into(),
            String::new(),
        ),
    };
    Ok(ReceiveOutcome::Event(PlatformEvent::Generic(
        GenericEvent {
            metadata: data(Value::Null, event_type, None, "generic:event", id).metadata,
            payload,
            defaults: GenericDefaults {
                title: request.header("x-nomail-title").into(),
                source: request.header("x-nomail-source").into(),
                level: request.header("x-nomail-level").into(),
                url: request.header("x-nomail-url").into(),
            },
        },
    )))
}

impl GenericEvent {
    pub fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    pub fn to_message(&self) -> IncomingMessage {
        match &self.payload {
            GenericPayload::Text(content) => message(
                first(&[self.defaults.title.clone(), "Webhook notification".into()]),
                content.clone(),
                self.defaults.source.clone(),
                &self.defaults.level,
                self.defaults.url.clone(),
            ),
            GenericPayload::Json(payload) => message(
                first(&[
                    text(payload, &["title"]),
                    self.metadata.event_type.clone(),
                    "Webhook notification".into(),
                ]),
                first(&[
                    text(payload, &["content"]),
                    text(payload, &["message"]),
                    text(payload, &["body"]),
                    payload.to_string(),
                ]),
                text(payload, &["source"]),
                &text(payload, &["level"]),
                text(payload, &["url"]),
            ),
        }
    }
}
