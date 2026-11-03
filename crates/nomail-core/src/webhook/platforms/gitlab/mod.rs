//! GitLab authentication policies, custom/default payloads and inbox mapping.
pub mod config;
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct GitlabEvent {
    pub instance: Option<String>,
    pub object_kind: Option<String>,
    pub project_path: Option<String>,
    pub action: Option<String>,
}

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
    let payload = object(request.body)?;
    let event_type = required(
        request.header("x-gitlab-event").into(),
        "missing GitLab event type",
    )?;
    let event = GitlabEvent {
        instance: optional(request.header("x-gitlab-instance")),
        object_kind: optional(text(&payload, &["object_kind"])),
        project_path: optional(text(&payload, &["project", "path_with_namespace"])),
        action: optional(text(&payload, &["object_attributes", "action"])),
    };
    Ok(PlatformEvent {
        metadata: metadata(
            event_type,
            None,
            "gitlab:message",
            first(&[
                request.header("webhook-id").into(),
                request.header("idempotency-key").into(),
            ]),
        ),
        data: PlatformData::Gitlab(event),
        raw: EventContent::Json(payload),
    }
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

impl GitlabEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&serde_json::Value::Null);
        message(
            format!("GitLab {}", metadata.event_type),
            first(&[
                text(p, &["object_attributes", "title"]),
                text(p, &["object_attributes", "note"]),
                raw.to_string(),
            ]),
            first(&[
                self.project_path.clone().unwrap_or_default(),
                "gitlab".into(),
            ]),
            "normal",
            text(p, &["object_attributes", "url"]),
        )
    }
}
