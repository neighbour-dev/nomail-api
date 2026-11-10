//! GitHub webhook authentication, JSON/form decoding, event model and inbox mapping.
pub mod config;
mod models;
pub use models::*;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ProtocolResponse, ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto,
    support::*,
};
use serde_json::Value;

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
    let event = GithubEvent::from_payload(&event_type, &payload);
    Ok(PlatformEvent {
        metadata: metadata(
            event_type,
            None,
            "github:delivery",
            request.header("x-github-delivery").into(),
        ),
        data: PlatformData::Github(Box::new(event)),
        raw: EventContent::Json(payload),
    }
    .into())
}

impl GithubEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&Value::Null);
        let kind = metadata.event_type.as_str();
        let repository = self
            .repository
            .as_ref()
            .and_then(|repo| repo.full_name.as_deref())
            .unwrap_or("");
        let mut title = format!("GitHub {kind}");
        for part in [
            self.action.clone().unwrap_or_default(),
            repository.to_owned(),
        ] {
            if !part.is_empty() {
                title.push_str(&format!(": {part}"));
            }
        }
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
            first(&[content, raw.to_string()]),
            first(&[
                repository.to_owned(),
                self.sender
                    .as_ref()
                    .and_then(|user| user.login.clone())
                    .unwrap_or_default(),
                "github".into(),
            ]),
            // Outcomes stay in workflow_run.conclusion, not NoMail priority.
            "normal",
            first(&[
                url,
                self.repository
                    .as_ref()
                    .and_then(|repo| repo.html_url.clone())
                    .unwrap_or_default(),
            ]),
        )
    }
}
