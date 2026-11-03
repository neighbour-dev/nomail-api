//! Slack authentication, Events API/interactions/commands and inbox mapping.
pub mod config;
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
pub enum SlackEvent {
    EventsApi(SlackCallback),
    Interaction(SlackInteraction),
    SlashCommand(SlackCommand),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SlackCallback {
    pub team_id: Option<String>,
    pub subtype: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SlackInteraction {
    pub team_id: Option<String>,
    pub user_id: Option<String>,
    pub actions: Vec<SlackAction>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SlackAction {
    pub action_id: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct SlackCommand {
    pub team_id: Option<String>,
    pub user_id: Option<String>,
    pub command: String,
    pub text: Option<String>,
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
    let (event, metadata, raw) = if is_form(request) {
        let fields = form(request.body)?;
        if let Some(encoded) = fields.get("payload").and_then(Value::as_str) {
            let payload = object(encoded.as_bytes())?;
            let event_type = required(text(&payload, &["type"]), "missing Slack interaction type")?;
            let event = SlackEvent::Interaction(SlackInteraction {
                team_id: optional(first(&[
                    text(&payload, &["team_id"]),
                    text(&payload, &["team", "id"]),
                ])),
                user_id: optional(text(&payload, &["user", "id"])),
                actions: payload["actions"]
                    .as_array()
                    .map(|actions| {
                        actions
                            .iter()
                            .map(|action| SlackAction {
                                action_id: optional(text(action, &["action_id"])),
                                value: optional(text(action, &["value"])),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                text: optional(text(&payload, &["text"])),
            });
            (
                event,
                metadata(event_type, None, "slack:interaction", String::new()),
                EventContent::Json(payload),
            )
        } else {
            let raw = EventContent::Form(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").to_owned()))
                    .collect(),
            );
            let payload = Value::Object(fields);
            let event_type = required(text(&payload, &["command"]), "missing Slack command")?;
            let event = SlackEvent::SlashCommand(SlackCommand {
                team_id: optional(text(&payload, &["team_id"])),
                user_id: optional(text(&payload, &["user_id"])),
                command: event_type.clone(),
                text: optional(text(&payload, &["text"])),
            });
            (
                event,
                metadata(event_type, None, "slack:command", String::new()),
                raw,
            )
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
                let event = SlackEvent::EventsApi(SlackCallback {
                    team_id: optional(first(&[
                        text(&payload, &["team_id"]),
                        text(&payload, &["team", "id"]),
                    ])),
                    subtype: optional(text(&payload, &["event", "subtype"])),
                    text: optional(first(&[
                        text(&payload, &["event", "text"]),
                        text(&payload, &["text"]),
                    ])),
                });
                (
                    event,
                    metadata(event_type, None, "slack:event", id),
                    EventContent::Json(payload),
                )
            }
            _ => return Err(WebhookError::Unsupported("unsupported Slack request type")),
        }
    };
    Ok(PlatformEvent {
        metadata,
        data: PlatformData::Slack(event),
        raw,
    }
    .into())
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
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let (team_id, text) = match self {
            Self::EventsApi(event) => (&event.team_id, &event.text),
            Self::Interaction(event) => (&event.team_id, &event.text),
            Self::SlashCommand(event) => (&event.team_id, &event.text),
        };
        message(
            format!("Slack {}", metadata.event_type),
            first(&[text.clone().unwrap_or_default(), raw.to_string()]),
            team_id.clone().unwrap_or_else(|| "slack".into()),
            "normal",
            String::new(),
        )
    }
}
