//! Gitea's native headers, decoded-JSON HMAC, event resources and inbox mapping.
pub mod config;
mod models;
pub use models::*;
#[cfg(test)]
mod tests;

use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde_json::Value;

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    // Gitea signs PayloadContent (JSON), never the encoded form body. Do not
    // accept GitHub/Gogs compatibility signatures in place of the native header.
    let signing = match &config.authentication {
        Authentication::None => None,
        Authentication::HmacSha256(secret) if !secret.is_empty() => {
            let signatures = request.headers.values("x-gitea-signature");
            if signatures.len() != 1 || signatures[0].len() != 64 {
                return Err(WebhookError::Unauthorized);
            }
            Some((*secret, crypto::hex_signature(signatures[0])?))
        }
        _ => return Err(invalid_auth()),
    };
    let decoded = if is_form(request) {
        let fields = form(request.body)?;
        Some(
            fields
                .get("payload")
                .and_then(Value::as_str)
                .ok_or(WebhookError::InvalidPayload("missing Gitea form payload"))?
                .to_owned(),
        )
    } else {
        None
    };
    let body = decoded
        .as_ref()
        .map_or(request.body, |payload| payload.as_bytes());
    if let Some((secret, signature)) = signing {
        crypto::hmac(secret.as_bytes(), &[body], &signature)?;
    }
    let payload = object(body)?;
    let event_type = required(
        request.header("x-gitea-event").into(),
        "missing Gitea event type",
    )?;
    let mut event = GiteaEvent::from_payload(&event_type, &payload);
    event.event_type = optional(request.header("x-gitea-event-type"));
    event.installation_target_type =
        optional(request.header("x-gitea-hook-installation-target-type"));
    Ok(PlatformEvent {
        metadata: metadata(
            event_type,
            None,
            "gitea:delivery",
            request.header("x-gitea-delivery").into(),
        ),
        data: PlatformData::Gitea(Box::new(event)),
        raw: EventContent::Json(payload),
    }
    .into())
}

impl GiteaEvent {
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
            .and_then(|r| r.full_name.clone())
            .unwrap_or_default();
        let mut title = format!("Gitea {kind}");
        for part in [self.action.clone().unwrap_or_default(), repository.clone()] {
            if !part.is_empty() {
                title.push_str(": ");
                title.push_str(&part);
            }
        }
        let (content, url) = match kind {
            "issues" => (
                text(p, &["issue", "title"]),
                text(p, &["issue", "html_url"]),
            ),
            "issue_comment" => (
                text(p, &["comment", "body"]),
                text(p, &["comment", "html_url"]),
            ),
            "pull_request" => (
                text(p, &["pull_request", "title"]),
                text(p, &["pull_request", "html_url"]),
            ),
            "pull_request_approved" | "pull_request_rejected" | "pull_request_comment" => (
                first(&[
                    text(p, &["review", "content"]),
                    text(p, &["pull_request", "title"]),
                ]),
                text(p, &["pull_request", "html_url"]),
            ),
            "push" => (
                text(p, &["head_commit", "message"]),
                text(p, &["compare_url"]),
            ),
            "create" | "delete" => (text(p, &["ref"]), String::new()),
            "fork" => (
                text(p, &["forkee", "full_name"]),
                text(p, &["forkee", "html_url"]),
            ),
            "wiki" => (
                first(&[text(p, &["comment"]), text(p, &["page"])]),
                String::new(),
            ),
            "release" => (
                first(&[
                    text(p, &["release", "name"]),
                    text(p, &["release", "tag_name"]),
                ]),
                text(p, &["release", "html_url"]),
            ),
            "package" => (
                first(&[
                    text(p, &["package", "name"]),
                    text(p, &["package", "version"]),
                ]),
                text(p, &["package", "html_url"]),
            ),
            "status" => (
                first(&[
                    text(p, &["description"]),
                    text(p, &["context"]),
                    text(p, &["state"]),
                ]),
                text(p, &["target_url"]),
            ),
            "workflow_run" => (
                first(&[
                    text(p, &["workflow_run", "display_title"]),
                    text(p, &["workflow", "name"]),
                ]),
                text(p, &["workflow_run", "html_url"]),
            ),
            "workflow_job" => (
                text(p, &["workflow_job", "name"]),
                text(p, &["workflow_job", "html_url"]),
            ),
            "repository" => (
                text(p, &["repository", "full_name"]),
                text(p, &["repository", "html_url"]),
            ),
            _ => (String::new(), String::new()),
        };
        message(
            title,
            first(&[content, raw.to_string()]),
            first(&[
                repository,
                self.sender
                    .as_ref()
                    .and_then(|u| u.login.clone())
                    .unwrap_or_default(),
                "gitea".into(),
            ]),
            "normal",
            first(&[url, text(p, &["repository", "html_url"])]),
        )
    }
}
