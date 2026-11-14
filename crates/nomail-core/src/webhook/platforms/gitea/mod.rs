//! Gitea's native headers, decoded-JSON HMAC, event resources and inbox mapping.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] = &["x-gitea-signature"];
mod models;
pub use models::*;
#[cfg(test)]
mod tests;

use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use serde_json::Value;

webhook_header!(GiteaWebhookHeader {
    event = "x-gitea-event", event_type = "x-gitea-event-type", delivery = "x-gitea-delivery",
    hook_id = "x-gitea-hook-id", installation_target_type = "x-gitea-hook-installation-target-type",
});
webhook!(GiteaWebhook, GiteaWebhookHeader, GiteaWebhookPayload);

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
    let payload = json_payload(body)?;
    let event_type = request.header("x-gitea-event").trim().to_owned();
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(
            event_type,
            None,
            "gitea:delivery",
            request.header("x-gitea-delivery").into(),
        ),
        EventContent::Json(payload),
        request,
    )
    .into())
}

impl GiteaWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&Value::Null);
        let kind = metadata.event_type.as_str();
        let repository = text(p, &["repository", "full_name"]);
        let mut title = format!("Gitea {kind}");
        for part in [text(p, &["action"]), repository.clone()] {
            if !part.is_empty() {
                title.push_str(": ");
                title.push_str(&part);
            }
        }
        use super::JsonFieldExt;
        let model = &self.payload;
        let comment = model.comment.value().and_then(|v| match v {
            GiteaWebhookComment::Text(text) => Some(text.as_str()),
            GiteaWebhookComment::Comment(comment) => comment.body.value_deref(),
        });
        let preview = match kind {
            "issues" => summary(&[
                model.issue.value().and_then(|v| v.body.value_deref()),
                model.issue.value().and_then(|v| v.title.value_deref()),
            ]),
            "issue_comment" => summary(&[comment]),
            "pull_request" => summary(&[
                model
                    .pull_request
                    .value()
                    .and_then(|v| v.body.value_deref()),
                model
                    .pull_request
                    .value()
                    .and_then(|v| v.title.value_deref()),
            ]),
            "pull_request_approved" | "pull_request_rejected" | "pull_request_comment" => {
                summary(&[
                    model.review.value().and_then(|v| v.content.value_deref()),
                    model
                        .pull_request
                        .value()
                        .and_then(|v| v.title.value_deref()),
                ])
            }
            "push" => summary(&[model
                .head_commit
                .value()
                .and_then(|v| v.message.value_deref())]),
            "create" | "delete" => summary(&[model.ref_name.value_deref()]),
            "fork" => summary(&[model.forkee.value().and_then(|v| v.full_name.value_deref())]),
            "wiki" => summary(&[comment, model.page.value_deref()]),
            "release" => summary(&[
                model.release.value().and_then(|v| v.body.value_deref()),
                model.release.value().and_then(|v| v.name.value_deref()),
                model.release.value().and_then(|v| v.tag_name.value_deref()),
            ]),
            "package" => summary(&[
                model.package.value().and_then(|v| v.name.value_deref()),
                model.package.value().and_then(|v| v.version.value_deref()),
            ]),
            "status" => summary(&[
                model.description.value_deref(),
                model.context.value_deref(),
                model.state.value_deref(),
            ]),
            "workflow_run" => summary(&[
                model
                    .workflow_run
                    .value()
                    .and_then(|v| v.display_title.value_deref()),
                model.workflow.value().and_then(|v| v.name.value_deref()),
            ]),
            "workflow_job" => summary(&[model
                .workflow_job
                .value()
                .and_then(|v| v.name.value_deref())]),
            "repository" => summary(&[model
                .repository
                .value()
                .and_then(|v| v.full_name.value_deref())]),
            _ => String::new(),
        };
        let url = match kind {
            "issues" => text(p, &["issue", "html_url"]),
            "issue_comment" => text(p, &["comment", "html_url"]),
            "pull_request"
            | "pull_request_approved"
            | "pull_request_rejected"
            | "pull_request_comment" => text(p, &["pull_request", "html_url"]),
            "push" => text(p, &["compare_url"]),
            "fork" => text(p, &["forkee", "html_url"]),
            "release" => text(p, &["release", "html_url"]),
            "package" => text(p, &["package", "html_url"]),
            "status" => text(p, &["target_url"]),
            "workflow_run" => text(p, &["workflow_run", "html_url"]),
            "workflow_job" => text(p, &["workflow_job", "html_url"]),
            _ => String::new(),
        };
        message(
            title,
            preview,
            "normal",
            first(&[url, text(p, &["repository", "html_url"])]),
        )
    }
}
