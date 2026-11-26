//! Explicit HTTP/JSON contract; no dependency on the private relay repository.
use super::{Channel, Job};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

const RELAY_ORIGIN: &str = "https://push.nomail.sg";
pub const TOKEN_SETTING: &str = "NOMAIL_PUSH_TOKEN";
pub const MAX_REPLY_BYTES: usize = 16 * 1024;
pub const TIMEOUT_SECONDS: u64 = 5;
const DELIVERIES: &str = "/v1/caller/deliveries";

#[derive(Clone)]
pub struct RelayConfig {
    origin: Url,
    token: String,
}

impl RelayConfig {
    pub fn from_token(token: String) -> Result<Option<Self>, &'static str> {
        if token.is_empty() {
            return Ok(None);
        }
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("NOMAIL_PUSH_TOKEN must be a 64-character instance token");
        }
        let origin = Url::parse(RELAY_ORIGIN).expect("valid official push relay origin");
        Ok(Some(Self { origin, token }))
    }

    /// Injects a loopback HTTP server in the native integration tests only.
    #[doc(hidden)]
    pub fn for_local_test(addr: std::net::SocketAddr, token: String) -> Self {
        assert!(addr.ip().is_loopback());
        let mut config = Self::from_token(token)
            .expect("valid dummy token")
            .expect("dummy token");
        config.origin = Url::parse(&format!("http://{addr}")).expect("valid loopback origin");
        config
    }

    pub fn url(&self, request: &RelayRequest) -> Url {
        let mut url = self.origin.clone();
        match request {
            RelayRequest::Submit(_) => url.set_path(DELIVERIES),
            RelayRequest::Status(id) => url.set_path(&format!("{DELIVERIES}/{id}")),
        }
        url
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

pub enum RelayRequest<'a> {
    Submit(Command<'a>),
    Status(Uuid),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Command<'a> {
    delivery_id: Uuid,
    message_id: Uuid,
    notification: Notification<'a>,
    target: Target<'a>,
}
#[derive(Serialize)]
struct Notification<'a> {
    title: &'a str,
    body: &'a str,
}
#[derive(Serialize)]
struct Target<'a> {
    channel: Channel,
    token: &'a str,
}

impl Job {
    pub fn request(&self) -> RelayRequest<'_> {
        if self.accepted {
            RelayRequest::Status(self.delivery_id)
        } else {
            RelayRequest::Submit(Command {
                delivery_id: self.delivery_id,
                message_id: self.message_id,
                notification: Notification {
                    title: &self.title,
                    body: &self.body,
                },
                target: Target {
                    channel: self.channel,
                    token: &self.token,
                },
            })
        }
    }
}

pub struct Reply {
    pub status: u16,
    pub retry_after: Option<String>,
    pub body: Vec<u8>,
}

/// None means the transport/response could not be read; the result is unknown.
#[allow(async_fn_in_trait)]
pub trait Transport {
    async fn request(&self, request: RelayRequest<'_>) -> Option<Reply>;
}

#[derive(Deserialize)]
struct Envelope {
    code: u16,
    data: serde_json::Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Delivery {
    delivery_id: Uuid,
    status: String,
    last_error: Option<ChannelError>,
}
#[derive(Deserialize)]
struct ChannelError {
    code: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Decision {
    Accepted,
    Done,
    InvalidTarget,
    Poll,
    Rejected,
    Retry(i64),
    AuthorizationBlocked(i64),
}

pub(super) fn decision(job: &Job, reply: Option<Reply>, now: i64) -> Decision {
    let Some(reply) = reply else {
        return Decision::Retry(0);
    };
    let parsed = serde_json::from_slice::<Envelope>(&reply.body);
    let retry_after = reply
        .retry_after
        .as_deref()
        .and_then(|v| {
            v.trim().parse::<i64>().ok().or_else(|| {
                DateTime::parse_from_rfc2822(v)
                    .ok()
                    .map(|date| date.timestamp().saturating_sub(now))
            })
        })
        .unwrap_or(0);
    if reply.status == 429
        || matches!(reply.status, 500 | 502..=504)
            && !parsed
                .as_ref()
                .is_ok_and(|e| e.data["retryable"].as_bool() == Some(false))
    {
        return Decision::Retry(retry_after);
    }
    // A rotated credential or temporarily exhausted entitlement must not
    // invalidate device registration. Wait for operator action within the deadline.
    if reply.status == 401 || parsed.as_ref().is_ok_and(|e| matches!(e.code, 2002..=2004)) {
        return Decision::AuthorizationBlocked(retry_after.max(60));
    }
    if reply.status != 200 && (job.accepted || reply.status != 202) {
        return Decision::Rejected;
    }
    let Ok(envelope) = parsed else {
        return Decision::Retry(retry_after);
    };
    if envelope.code != 0 {
        return if envelope.data["retryable"].as_bool() == Some(true) {
            Decision::Retry(retry_after)
        } else {
            Decision::Rejected
        };
    }
    let Ok(delivery) = serde_json::from_value::<Delivery>(envelope.data) else {
        return Decision::Retry(retry_after);
    };
    if delivery.delivery_id != job.delivery_id {
        return Decision::Retry(retry_after);
    }
    match delivery.status.as_str() {
        "provider_accepted" => Decision::Done,
        "failed"
            if delivery
                .last_error
                .is_some_and(|e| e.code == "invalid_target") =>
        {
            Decision::InvalidTarget
        }
        "failed" => Decision::Rejected,
        _ if job.accepted => Decision::Poll,
        _ => Decision::Accepted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn job(accepted: bool) -> Job {
        Job {
            delivery_id: Uuid::from_u128(1),
            message_id: Uuid::from_u128(2),
            channel: Channel::Apns,
            token: "abcdef".into(),
            title: crate::push::TITLE.into(),
            body: crate::push::BODY.into(),
            accepted,
            attempt: 1,
            lease: 1,
        }
    }

    fn reply(status: u16, code: u16, data: Value) -> Option<Reply> {
        Some(Reply {
            status,
            retry_after: Some("12".into()),
            body: serde_json::to_vec(&json!({"code":code,"data":data})).unwrap(),
        })
    }

    #[test]
    fn validates_connection_configuration_and_builds_caller_paths() {
        assert!(RelayConfig::from_token(String::new()).unwrap().is_none());
        let token = "ab".repeat(32);
        for bad in ["short".to_string(), "g".repeat(64), " ".repeat(64)] {
            assert!(RelayConfig::from_token(bad).is_err());
        }
        let config = RelayConfig::from_token(token.clone()).unwrap().unwrap();
        assert_eq!(config.token(), token);
        assert_eq!(
            config.url(&job(false).request()).as_str(),
            "https://push.nomail.sg/v1/caller/deliveries"
        );
        assert_eq!(
            config.url(&job(true).request()).as_str(),
            format!(
                "https://push.nomail.sg{DELIVERIES}/{}",
                job(true).delivery_id
            )
        );
    }

    #[test]
    fn retains_job_on_unknown_response_and_distinguishes_replay_from_rejection() {
        let job = job(false);
        let queued = json!({"deliveryId":job.delivery_id,"status":"queued"});
        for status in [200, 202] {
            assert_eq!(
                decision(&job, reply(status, 0, queued.clone()), 0),
                Decision::Accepted
            );
        }
        assert_eq!(decision(&job, None, 0), Decision::Retry(0));
        assert_eq!(
            decision(&job, reply(503, 5001, json!({"retryable":true})), 0),
            Decision::Retry(12)
        );
        assert_eq!(
            decision(&job, reply(200, 2004, json!({"retryable":false})), 0),
            Decision::AuthorizationBlocked(60)
        );
        assert_eq!(
            decision(&job, reply(401, 2001, json!({"retryable":false})), 0),
            Decision::AuthorizationBlocked(60)
        );
        assert_eq!(
            decision(&job, reply(409, 3003, json!({"retryable":false})), 0),
            Decision::Rejected
        );
        assert_eq!(
            decision(
                &job,
                reply(
                    202,
                    0,
                    json!({"deliveryId":job.message_id,"status":"queued"})
                ),
                0
            ),
            Decision::Retry(12)
        );
    }

    #[test]
    fn status_invalidates_only_an_explicit_terminal_channel_error() {
        let job = job(true);
        let update = |status, error| {
            reply(
                200,
                0,
                json!({
                    "deliveryId":job.delivery_id,"status":status,"lastError":{"code":error}
                }),
            )
        };
        assert_eq!(
            decision(&job, update("retry_wait", "channel_rate_limited"), 0),
            Decision::Poll
        );
        assert_eq!(
            decision(&job, update("provider_accepted", "ignored"), 0),
            Decision::Done
        );
        assert_eq!(
            decision(&job, update("failed", "provider_configuration"), 0),
            Decision::Rejected
        );
        assert_eq!(
            decision(&job, update("failed", "invalid_target"), 0),
            Decision::InvalidTarget
        );
    }
}
