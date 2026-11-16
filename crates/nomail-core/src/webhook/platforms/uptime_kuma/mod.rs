//! Uptime Kuma's default JSON webhook, which has no native authentication.
//! Independently modeled from upstream; no upstream implementation is copied.
pub mod config;
#[cfg(test)]
mod tests;

use crate::webhook::{
    Authentication, EventContent, EventMetadata, JsonFieldExt, MessageContent, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, support::*,
};

webhook_header!(UptimeKumaWebhookHeader {});
webhook!(
    UptimeKumaWebhook,
    UptimeKumaWebhookHeader,
    UptimeKumaWebhookPayload
);

native_model!(UptimeKumaWebhookPayload {
    heartbeat: UptimeKumaHeartbeat = "heartbeat",
    monitor: UptimeKumaMonitor = "monitor",
    msg: String = "msg",
});

native_model!(UptimeKumaHeartbeat {
    monitor_id: i64 = "monitorID",
    status: i64 = "status",
    time: String = "time",
    msg: String = "msg",
    ping: serde_json::Number = "ping",
    important: bool = "important",
    duration: serde_json::Number = "duration",
    retries: i64 = "retries",
    response: String = "response",
    timezone: String = "timezone",
    timezone_offset: String = "timezoneOffset",
    local_date_time: String = "localDateTime",
    last_down_time: String = "lastDownTime",
});

native_model!(UptimeKumaMonitor {
    id: i64 = "id",
    name: String = "name",
    description: String = "description",
    monitor_type: String = "type",
    url: String = "url",
    hostname: String = "hostname",
    port: i64 = "port",
    method: String = "method",
    interval: serde_json::Number = "interval",
    retry_interval: serde_json::Number = "retryInterval",
    resend_interval: serde_json::Number = "resendInterval",
    timeout: serde_json::Number = "timeout",
    max_retries: i64 = "maxretries",
    active: bool = "active",
    upside_down: bool = "upsideDown",
});

fn status_name(status: Option<i64>) -> &'static str {
    match status {
        Some(0) => "down",
        Some(1) => "up",
        Some(2) => "pending",
        Some(3) => "maintenance",
        _ => "notification",
    }
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    let Authentication::None = &config.authentication else {
        return Err(invalid_auth());
    };
    let payload = json_payload(request.body)?;
    let kind = status_name(
        payload
            .get("heartbeat")
            .and_then(|h| h.get("status"))
            .and_then(serde_json::Value::as_i64),
    );
    Ok(PlatformEvent::from_raw(
        config.provider,
        // Default Kuma payloads have no unique event/delivery ID. In particular,
        // monitorID must not suppress repeated outages or reminder notifications.
        metadata(kind.into(), None, "uptimekuma:event", String::new()),
        EventContent::Json(payload),
        request,
    )
    .into())
}

impl UptimeKumaWebhook {
    pub fn to_message_content(
        &self,
        _metadata: &EventMetadata,
        _raw: &EventContent,
    ) -> MessageContent {
        let heartbeat = self.payload.heartbeat.value();
        let monitor = self.payload.monitor.value();
        let status = status_name(heartbeat.and_then(|h| h.status).flatten());
        let name = monitor
            .and_then(|m| m.name.value_deref())
            .filter(|s| !s.trim().is_empty());
        let title = match name {
            Some(name) => format!("Uptime Kuma: {name} ({status})"),
            None => format!("Uptime Kuma: {status}"),
        };
        message(
            title,
            summary(&[
                self.payload.msg.value_deref(),
                heartbeat.and_then(|h| h.msg.value_deref()),
            ]),
            // Result status is not automatically an attention priority.
            "normal",
            monitor
                .and_then(|m| m.url.value_deref())
                .unwrap_or("")
                .into(),
        )
    }
}
