//! Telegram Bot API authentication, Update model and inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventMetadata, IncomingMessage, PlatformEvent, PlatformPayload, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelegramEvent {
    pub data: PlatformPayload,
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::TelegramSecretToken(secret) if !secret.is_empty() => crypto::equal(
            secret.as_bytes(),
            request.header("x-telegram-bot-api-secret-token").as_bytes(),
        )?,
        _ => return Err(invalid_auth()),
    }
    let payload = object(request.body)?;
    let id = payload["update_id"]
        .as_i64()
        .ok_or(WebhookError::InvalidPayload("missing Telegram update_id"))?;
    let fields: Vec<_> = payload
        .as_object()
        .expect("object was checked")
        .keys()
        .filter(|key| key.as_str() != "update_id")
        .cloned()
        .collect();
    // Preserve unknown single update fields when Telegram adds new types.
    if fields.len() != 1 || id < 0 {
        return Err(WebhookError::InvalidPayload("invalid Telegram Update"));
    }
    Ok(ReceiveOutcome::Event(PlatformEvent::Telegram(
        TelegramEvent {
            data: data(
                payload,
                fields[0].clone(),
                None,
                "telegram:bot-update",
                id.to_string(),
            ),
        },
    )))
}

impl TelegramEvent {
    pub fn metadata(&self) -> &EventMetadata {
        &self.data.metadata
    }

    pub fn to_message(&self) -> IncomingMessage {
        let kind = &self.data.metadata.event_type;
        let update = &self.data.payload[kind];
        message(
            format!("Telegram {kind}"),
            first(&[
                text(update, &["text"]),
                text(update, &["caption"]),
                text(update, &["data"]),
                update.to_string(),
            ]),
            first(&[
                text(update, &["chat", "title"]),
                text(update, &["chat", "username"]),
                "telegram".into(),
            ]),
            "normal",
            String::new(),
        )
    }
}
