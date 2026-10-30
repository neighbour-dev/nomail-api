//! GitHub webhook authentication, JSON/form decoding, event model and inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventMetadata, IncomingMessage, PlatformEvent, PlatformPayload,
    ProtocolResponse, ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto,
    support::*,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GithubEvent {
    pub data: PlatformPayload,
    pub action: Option<String>,
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::HmacSha256(secret) if !secret.is_empty() => {
            crypto::hmac_header(secret, "x-hub-signature-256", request)?
        }
        _ => return Err(invalid_auth()),
    }
    let payload = if is_form(request) {
        let fields = form(request.body)?;
        let encoded = fields
            .get("payload")
            .and_then(Value::as_str)
            .ok_or(WebhookError::InvalidPayload("missing GitHub form payload"))?;
        object(encoded.as_bytes())?
    } else {
        object(request.body)?
    };
    let event_type = required(
        request.header("x-github-event").into(),
        "missing GitHub event type",
    )?;
    if event_type == "ping" {
        return Ok(ReceiveOutcome::Respond(ProtocolResponse {
            status: 200,
            content_type: "text/plain".into(),
            body: b"pong".to_vec(),
        }));
    }
    Ok(ReceiveOutcome::Event(PlatformEvent::Github(GithubEvent {
        action: optional(text(&payload, &["action"])),
        data: data(
            payload,
            event_type,
            None,
            "github:delivery",
            request.header("x-github-delivery").into(),
        ),
    })))
}

impl GithubEvent {
    pub fn metadata(&self) -> &EventMetadata {
        &self.data.metadata
    }

    pub fn to_message(&self) -> IncomingMessage {
        let p = &self.data.payload;
        let kind = self.data.metadata.event_type.as_str();
        let mut title = format!("GitHub {kind}");
        for part in [
            self.action.clone().unwrap_or_default(),
            text(p, &["repository", "full_name"]),
        ] {
            if !part.is_empty() {
                title.push_str(&format!(": {part}"));
            }
        }
        let conclusion = text(p, &["workflow_run", "conclusion"]);
        let level = match conclusion.as_str() {
            "failure" | "cancelled" | "timed_out" | "startup_failure" => "critical",
            "success" => "success",
            _ => "normal",
        };
        let (content, url) = match kind {
            "issues" => (
                text(p, &["issue", "title"]),
                text(p, &["issue", "html_url"]),
            ),
            "issue_comment" | "pull_request_review_comment" => (
                text(p, &["comment", "body"]),
                text(p, &["comment", "html_url"]),
            ),
            "pull_request" | "pull_request_review" => (
                text(p, &["pull_request", "title"]),
                text(p, &["pull_request", "html_url"]),
            ),
            "push" => (text(p, &["head_commit", "message"]), text(p, &["compare"])),
            "workflow_run" => (
                text(p, &["workflow_run", "name"]),
                text(p, &["workflow_run", "html_url"]),
            ),
            _ => (String::new(), String::new()),
        };
        message(
            title,
            first(&[content, summary(&self.data)]),
            first(&[
                text(p, &["repository", "full_name"]),
                text(p, &["sender", "login"]),
                "github".into(),
            ]),
            level,
            first(&[url, text(p, &["repository", "html_url"])]),
        )
    }
}
