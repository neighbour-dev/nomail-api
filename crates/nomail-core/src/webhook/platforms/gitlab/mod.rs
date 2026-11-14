//! GitLab authentication policies, custom/default payloads and inbox mapping.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] =
    &["x-gitlab-token", "webhook-signature"];
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;

webhook_header!(GitlabWebhookHeader {
    event = "x-gitlab-event", instance = "x-gitlab-instance",
    event_uuid = "x-gitlab-event-uuid", webhook_uuid = "x-gitlab-webhook-uuid",
    webhook_id = "webhook-id", idempotency_key = "idempotency-key",
});
native_model!(GitlabWebhookPayload {
    object_kind: String = "object_kind", event_name: String = "event_name",
    project: GitlabProject = "project", object_attributes: Value = "object_attributes",
    user: Value = "user", commits: Vec<Value> = "commits", ref_name: String = "ref",
    before: String = "before", after: String = "after",
    // Custom templates can contain arbitrary JSON at native root paths.
    title: String = "title", message: String = "message",
});
native_model!(GitlabProject {
    id: i64 = "id",
    name: String = "name",
    path_with_namespace: String = "path_with_namespace",
    web_url: String = "web_url",
});
webhook!(GitlabWebhook, GitlabWebhookHeader, GitlabWebhookPayload);

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::GitlabToken(secret) if !secret.is_empty() => crypto::equal(
            secret.as_bytes(),
            request.header("x-gitlab-token").as_bytes(),
        )?,
        Authentication::GitlabSigningToken(secret) if !secret.is_empty() => {
            verify_signature(config, request, secret)?
        }
        _ => return Err(invalid_auth()),
    }
    let payload = json_payload(request.body)?;
    let event_type = request.header("x-gitlab-event").trim().to_owned();
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(
            event_type,
            None,
            "gitlab:message",
            first(&[
                request.header("webhook-id").into(),
                request.header("idempotency-key").into(),
            ]),
        ),
        EventContent::Json(payload),
        request,
    )
    .into())
}

fn verify_signature(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
    secret: &str,
) -> Result<(), WebhookError> {
    let key = secret
        .strip_prefix("whsec_")
        .ok_or(WebhookError::InvalidConfiguration(
            "invalid GitLab signing token",
        ))?;
    let key = STANDARD
        .decode(key)
        .map_err(|_| WebhookError::InvalidConfiguration("invalid GitLab signing token"))?;
    if key.is_empty() {
        return Err(WebhookError::InvalidConfiguration(
            "empty GitLab signing key",
        ));
    }
    let id = request.header("webhook-id");
    let time = request.header("webhook-timestamp");
    if id.is_empty() {
        return Err(WebhookError::Unauthorized);
    }
    crypto::timestamp(config, time)?;
    for value in request.headers.values("webhook-signature") {
        for candidate in value.split_whitespace() {
            if let Some(encoded) = candidate.strip_prefix("v1,")
                && let Ok(signature) = STANDARD.decode(encoded)
                && crypto::hmac(
                    &key,
                    &[id.as_bytes(), b".", time.as_bytes(), b".", request.body],
                    &signature,
                )
                .is_ok()
            {
                return Ok(());
            }
        }
    }
    Err(WebhookError::Unauthorized)
}

impl GitlabWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&serde_json::Value::Null);
        use super::JsonFieldExt;
        let attrs = self.payload.object_attributes.value();
        message(
            format!("GitLab {}", metadata.event_type),
            summary(&[
                attrs.and_then(|v| v.get("note")).and_then(Value::as_str),
                attrs
                    .and_then(|v| v.get("description"))
                    .and_then(Value::as_str),
                attrs.and_then(|v| v.get("title")).and_then(Value::as_str),
                self.payload.message.value_deref(),
                self.payload.title.value_deref(),
            ]),
            "normal",
            text(p, &["object_attributes", "url"]),
        )
    }
}
