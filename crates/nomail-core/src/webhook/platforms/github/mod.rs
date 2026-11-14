//! GitHub webhook authentication, JSON/form decoding, event model and inbox mapping.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] =
    &["x-hub-signature", "x-hub-signature-256"];
#[macro_use]
mod support;
mod generated;
mod models;
pub use generated::*;
pub use models::*;
pub use support::GithubNumber;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ProtocolResponse,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde_json::Value;

webhook_header!(GithubWebhookHeader {
    event = "x-github-event", delivery = "x-github-delivery", hook_id = "x-github-hook-id",
    hook_installation_target_id = "x-github-hook-installation-target-id",
    hook_installation_target_type = "x-github-hook-installation-target-type",
});
webhook!(GithubWebhook, GithubWebhookHeader, GithubWebhookPayload);

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
        json_payload(encoded.as_bytes())?
    } else {
        json_payload(request.body)?
    };
    let event_type = request.header("x-github-event").trim().to_owned();
    if event_type == "ping" {
        return Ok(ReceiveOutcome::Respond(ProtocolResponse {
            status: 200,
            content_type: "text/plain".into(),
            body: b"pong".to_vec(),
        }));
    }
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(
            event_type,
            None,
            "github:delivery",
            request.header("x-github-delivery").into(),
        ),
        EventContent::Json(payload),
        request,
    )
    .into())
}

impl GithubWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&Value::Null);
        let kind = metadata.event_type.as_str();
        let repository = text(p, &["repository", "full_name"]);
        let mut title = format!("GitHub {kind}");
        for part in [text(p, &["action"]), repository.to_owned()] {
            if !part.is_empty() {
                title.push_str(&format!(": {part}"));
            }
        }
        let p_model = &self.payload;
        use super::JsonFieldExt;
        let comment = p_model.comment.value().and_then(|c| match c {
            GithubCommentValue::Comment(c) => c.body.value_deref(),
            GithubCommentValue::Text(text) => Some(text.as_str()),
        });
        let preview = match kind {
            "issues" => summary(&[
                p_model.issue.value().and_then(|v| v.body.value_deref()),
                p_model.issue.value().and_then(|v| v.title.value_deref()),
            ]),
            "issue_comment" | "pull_request_review_comment" => summary(&[comment]),
            "pull_request" => summary(&[
                p_model
                    .pull_request
                    .value()
                    .and_then(|v| v.issue.body.value_deref()),
                p_model
                    .pull_request
                    .value()
                    .and_then(|v| v.issue.title.value_deref()),
            ]),
            "pull_request_review" => {
                summary(&[p_model.review.value().and_then(|v| v.body.value_deref())])
            }
            "push" => summary(&[p_model
                .head_commit
                .value()
                .and_then(|v| v.message.value_deref())]),
            "workflow_run" => summary(&[p_model
                .workflow_run
                .value()
                .and_then(|v| v.name.value_deref())]),
            "workflow_job" => summary(&[p_model
                .workflow_job
                .value()
                .and_then(|v| v.name.value_deref())]),
            "release" => summary(&[
                p_model.release.value().and_then(|v| v.body.value_deref()),
                p_model.release.value().and_then(|v| v.name.value_deref()),
            ]),
            "discussion" => summary(&[
                p_model
                    .discussion
                    .value()
                    .and_then(|v| v.body.value_deref()),
                p_model
                    .discussion
                    .value()
                    .and_then(|v| v.title.value_deref()),
            ]),
            _ => String::new(),
        };
        let url = match kind {
            "issues" => text(p, &["issue", "html_url"]),
            "issue_comment" | "pull_request_review_comment" => text(p, &["comment", "html_url"]),
            "pull_request" | "pull_request_review" => text(p, &["pull_request", "html_url"]),
            "push" => text(p, &["compare"]),
            "workflow_run" => text(p, &["workflow_run", "html_url"]),
            _ => String::new(),
        };
        message(
            title,
            preview,
            // Outcomes stay in workflow_run.conclusion, not NoMail priority.
            "normal",
            first(&[url, text(p, &["repository", "html_url"])]),
        )
    }
}
