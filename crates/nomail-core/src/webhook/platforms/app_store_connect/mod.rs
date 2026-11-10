//! App Store Connect HTTP notification authentication, projection and summaries.
//! Sources: developer.apple.com/documentation/appstoreconnectapi/webhook-events
//! and /configuring-webhook-notifications. No API lookups or IAP/JWS handling.
pub mod config;
mod models;
pub use models::*;
#[cfg(test)]
mod tests;

use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};

fn verify_signature(secret: &str, request: &WebhookRequest<'_>) -> Result<(), WebhookError> {
    let values = request.headers.values("x-apple-signature");
    if values.len() != 1 {
        return Err(WebhookError::Unauthorized);
    }
    let signature = values[0]
        .strip_prefix("hmacsha256=")
        .ok_or(WebhookError::Unauthorized)?;
    if signature.len() != 64 {
        return Err(WebhookError::Unauthorized);
    }
    crypto::hmac(
        secret.as_bytes(),
        &[request.body],
        &crypto::hex_signature(signature)?,
    )
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    match &config.authentication {
        Authentication::None => {}
        Authentication::HmacSha256(secret) if !secret.is_empty() => {
            verify_signature(secret, request)?
        }
        _ => return Err(invalid_auth()),
    }
    let payload = object(request.body)?;
    let event = AppStoreConnectEvent::from_payload(&payload)?;
    // Apple redeliveries share the event ID; the related instance can be shared
    // by many distinct state-change events and must never be the dedup key.
    let metadata = metadata(
        event.data.event_type.clone(),
        event.data.version.map(|v| v.to_string()),
        "appstoreconnect:event",
        event.data.id.clone().unwrap_or_default(),
    );
    Ok(PlatformEvent {
        metadata,
        data: PlatformData::AppStoreConnect(Box::new(event)),
        raw: EventContent::Json(payload),
    }
    .into())
}

fn transition(old: Option<&str>, new: Option<&str>) -> String {
    match (old.filter(|s| !s.is_empty()), new.filter(|s| !s.is_empty())) {
        (Some(old), Some(new)) => format!("{old} → {new}"),
        (_, Some(new)) => new.to_owned(),
        (Some(old), _) => old.to_owned(),
        _ => String::new(),
    }
}

impl AppStoreConnectEvent {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let empty = AppStoreConnectAttributes::default();
        let attrs = self.data.attributes.as_ref().unwrap_or(&empty);
        let (label, content) = match metadata.event_type.as_str() {
            "appStoreVersionAppVersionStateUpdated" => (
                "App version state updated",
                transition(attrs.old_value.as_deref(), attrs.new_value.as_deref()),
            ),
            "buildBetaDetailExternalBuildStateUpdated" => (
                "External beta state updated",
                transition(
                    attrs.old_external_build_state.as_deref(),
                    attrs.new_external_build_state.as_deref(),
                ),
            ),
            "buildUploadStateUpdated" => (
                "Build upload state updated",
                transition(attrs.old_state.as_deref(), attrs.new_state.as_deref()),
            ),
            "backgroundAssetVersionStateUpdated" => (
                "Background asset state updated",
                transition(attrs.old_state.as_deref(), attrs.new_state.as_deref()),
            ),
            "backgroundAssetVersionExternalBetaReleaseStateUpdated" => (
                "Background asset external beta state updated",
                transition(attrs.old_state.as_deref(), attrs.new_state.as_deref()),
            ),
            "backgroundAssetVersionAppStoreReleaseStateUpdated" => (
                "Background asset App Store release state updated",
                transition(attrs.old_state.as_deref(), attrs.new_state.as_deref()),
            ),
            "backgroundAssetVersionInternalBetaReleaseCreated" => (
                "Background asset internal beta release created",
                "Internal beta release created".into(),
            ),
            "betaFeedbackScreenshotSubmissionCreated" => (
                "TestFlight screenshot feedback created",
                "Screenshot feedback created".into(),
            ),
            "betaFeedbackCrashSubmissionCreated" => (
                "TestFlight crash feedback created",
                "Crash feedback created".into(),
            ),
            "alternativeDistributionPackageVersionCreated" => (
                "Alternative distribution package version created",
                "Package version created".into(),
            ),
            "alternativeDistributionPackageAvailableUpdated"
            | "alternativeDistributionTerritoryAvailabilityUpdated" => {
                let availability = attrs
                    .available
                    .map(|v| if v { "Available" } else { "Unavailable" })
                    .unwrap_or("Availability updated");
                let content = if attrs.territories.is_empty() {
                    availability.into()
                } else {
                    format!("{availability}: {}", attrs.territories.join(", "))
                };
                ("Alternative distribution availability updated", content)
            }
            _ => (metadata.event_type.as_str(), raw.to_string()),
        };
        let relationships = self.data.relationships.as_ref();
        let url = relationships
            .and_then(|r| r.instance.as_ref())
            .and_then(AppStoreConnectRelationship::self_url)
            .or_else(|| {
                relationships
                    .and_then(|r| r.marketplace_app.as_ref())
                    .and_then(AppStoreConnectRelationship::self_url)
            });
        message(
            format!("App Store Connect: {label}"),
            first(&[content, raw.to_string()]),
            "App Store Connect".into(),
            "normal",
            url.unwrap_or_default().into(),
        )
    }
}
