//! Slack authentication, Events API/interactions/commands and inbox mapping.
pub mod config;
#[cfg(test)]
mod tests;
use super::JsonFieldExt;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde_json::Value;

webhook_header!(SlackWebhookHeader {
    retry_num = "x-slack-retry-num", retry_reason = "x-slack-retry-reason",
});
webhook!(SlackWebhook, SlackWebhookHeader, SlackWebhookPayload);
native_model!(SlackWebhookPayload {
    event_type: String = "type", event_id: String = "event_id", event_time: i64 = "event_time",
    team_id: String = "team_id", api_app_id: String = "api_app_id", event: SlackCallbackEvent = "event",
    team: SlackTeam = "team", user: SlackUser = "user", actions: Vec<SlackAction> = "actions",
    user_id: String = "user_id", command: String = "command", text: String = "text",
    channel_id: String = "channel_id", channel_name: String = "channel_name",
    team_domain: String = "team_domain", user_name: String = "user_name",
    view: Value = "view", message: Value = "message", container: Value = "container",
});
native_model!(SlackCallbackEvent {
    event_type: String = "type",
    subtype: String = "subtype",
    text: String = "text",
    user: String = "user",
    channel: String = "channel",
    ts: String = "ts",
    event_ts: String = "event_ts",
});
native_model!(SlackTeam {
    id: String = "id",
    domain: String = "domain"
});
native_model!(SlackUser {
    id: String = "id",
    username: String = "username",
    name: String = "name",
    team_id: String = "team_id"
});
native_model!(SlackAction {
    action_id: String = "action_id",
    value: String = "value",
    action_type: String = "type",
    block_id: String = "block_id",
});

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
    let (metadata, raw) = if is_form(request) {
        let fields: Vec<(String, String)> = serde_urlencoded::from_bytes(request.body)
            .map_err(|_| WebhookError::InvalidPayload("invalid form payload"))?;
        let mut interaction = fields.iter().filter(|(key, _)| key == "payload");
        let encoded = interaction.next().filter(|_| interaction.next().is_none());
        if let Some((_, encoded)) = encoded {
            let payload = json_payload(encoded.as_bytes())?;
            let event_type = text(&payload, &["type"]);
            (
                metadata(event_type, None, "slack:interaction", String::new()),
                EventContent::Json(payload),
            )
        } else {
            let event_type = fields
                .iter()
                .find(|(key, _)| key == "command")
                .map(|(_, value)| value.trim().to_owned())
                .unwrap_or_default();
            (
                metadata(event_type, None, "slack:command", String::new()),
                EventContent::Form(fields),
            )
        }
    } else {
        let payload = json_payload(request.body)?;
        match text(&payload, &["type"]).as_str() {
            "url_verification" => return challenge(&payload),
            "app_rate_limited" => {
                return Ok(ReceiveOutcome::Ignored {
                    reason: "Slack rate limit notification",
                });
            }
            "event_callback" => {
                let event_type = text(&payload, &["event", "type"]);
                let id = text(&payload, &["event_id"]);
                (
                    metadata(event_type, None, "slack:event", id),
                    EventContent::Json(payload),
                )
            }
            _ => (
                metadata(
                    text(&payload, &["type"]),
                    None,
                    "slack:event",
                    text(&payload, &["event_id"]),
                ),
                EventContent::Json(payload),
            ),
        }
    };
    Ok(PlatformEvent::from_raw(config.provider, metadata, raw, request).into())
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

impl SlackWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json();
        let team_id = p
            .map(|p| first(&[text(p, &["team_id"]), text(p, &["team", "id"])]))
            .unwrap_or_else(|| self.payload.team_id.value().cloned().unwrap_or_default());
        let content = p
            .map(|p| first(&[text(p, &["event", "text"]), text(p, &["text"])]))
            .unwrap_or_else(|| {
                self.payload
                    .text
                    .value()
                    .map(String::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            });
        message(
            format!("Slack {}", metadata.event_type),
            first(&[content, raw.to_string()]),
            first(&[team_id, "slack".into()]),
            "normal",
            String::new(),
        )
    }
}
