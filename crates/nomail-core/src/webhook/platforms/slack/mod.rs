//! Slack authentication, Events API/interactions/commands and inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventMetadata, IncomingMessage, PlatformEvent, PlatformPayload, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SlackEvent {
    EventsApi(PlatformPayload),
    Interaction(PlatformPayload),
    SlashCommand(PlatformPayload),
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::SlackSigningSecret(secret) if !secret.is_empty() => {
            verify_signature(config, request, secret)?
        }
        _ => return Err(invalid_auth()),
    }
    let event = if is_form(request) {
        let fields = form(request.body)?;
        if let Some(encoded) = fields.get("payload").and_then(Value::as_str) {
            let payload = object(encoded.as_bytes())?;
            let event_type = required(text(&payload, &["type"]), "missing Slack interaction type")?;
            SlackEvent::Interaction(data(
                payload,
                event_type,
                None,
                "slack:interaction",
                String::new(),
            ))
        } else {
            let payload = Value::Object(fields);
            let event_type = required(text(&payload, &["command"]), "missing Slack command")?;
            SlackEvent::SlashCommand(data(
                payload,
                event_type,
                None,
                "slack:command",
                String::new(),
            ))
        }
    } else {
        let payload = object(request.body)?;
        match text(&payload, &["type"]).as_str() {
            "url_verification" => return challenge(&payload),
            "app_rate_limited" => {
                return Ok(ReceiveOutcome::Ignored {
                    reason: "Slack rate limit notification",
                });
            }
            "event_callback" => {
                let event_type = required(
                    text(&payload, &["event", "type"]),
                    "missing Slack event type",
                )?;
                let id = text(&payload, &["event_id"]);
                SlackEvent::EventsApi(data(payload, event_type, None, "slack:event", id))
            }
            _ => return Err(WebhookError::Unsupported("unsupported Slack request type")),
        }
    };
    Ok(ReceiveOutcome::Event(PlatformEvent::Slack(event)))
}

fn verify_signature(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
    secret: &str,
) -> Result<(), WebhookError> {
    let time = request.header("x-slack-request-timestamp");
    crypto::timestamp(config, time)?;
    let signature = request
        .header("x-slack-signature")
        .strip_prefix("v0=")
        .ok_or(WebhookError::Unauthorized)?;
    crypto::hmac(
        secret.as_bytes(),
        &[b"v0:", time.as_bytes(), b":", request.body],
        &crypto::hex_signature(signature)?,
    )
}

impl SlackEvent {
    fn data(&self) -> &PlatformPayload {
        match self {
            Self::EventsApi(data) | Self::Interaction(data) | Self::SlashCommand(data) => data,
        }
    }

    pub fn metadata(&self) -> &EventMetadata {
        &self.data().metadata
    }

    pub fn to_message(&self) -> IncomingMessage {
        let data = self.data();
        let p = &data.payload;
        message(
            format!("Slack {}", data.metadata.event_type),
            first(&[
                text(p, &["event", "text"]),
                text(p, &["text"]),
                summary(data),
            ]),
            first(&[
                text(p, &["team_id"]),
                text(p, &["team", "id"]),
                "slack".into(),
            ]),
            "normal",
            String::new(),
        )
    }
}
