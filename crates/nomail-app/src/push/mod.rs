//! Device registration and the durable handoff to the optional push relay.
mod relay;
pub use relay::*;

use crate::auth::{AuthContext, AuthError, AuthResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TITLE: &str = "NoMail";
pub const BODY: &str = "你有一条新消息";
pub const SUBMIT_WINDOW: i64 = 10 * 60;
const STATUS_WINDOW: i64 = 24 * 60 * 60;
const LEASE: i64 = 60;

#[allow(async_fn_in_trait)]
pub trait DeviceStore {
    /// Atomically recheck the active session and update only its own target.
    /// None removes the registration. Reposting the same channel and token preserves its version.
    async fn set_target(
        &self,
        actor: &AuthContext,
        target: Option<&Registration>,
        now: DateTime<Utc>,
    ) -> AuthResult<()>;
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Apns,
    Fcm,
}

impl Channel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Apns => "apns",
            Self::Fcm => "fcm",
        }
    }
}

impl std::str::FromStr for Channel {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "apns" => Ok(Self::Apns),
            "fcm" => Ok(Self::Fcm),
            _ => Err(StoreError),
        }
    }
}

pub struct Registration {
    pub channel: Channel,
    pub token: String,
}

impl Registration {
    pub fn new(channel: Channel, token: String) -> AuthResult<Self> {
        let token = match channel {
            Channel::Apns => {
                if !(2..=512).contains(&token.len())
                    || !token.len().is_multiple_of(2)
                    || !token.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(AuthError::InvalidInput("invalid APNs device token".into()));
                }
                token.to_ascii_lowercase()
            }
            Channel::Fcm => {
                if !(1..=4096).contains(&token.len())
                    || !token.bytes().all(|b| b.is_ascii_graphic())
                {
                    return Err(AuthError::InvalidInput(
                        "invalid FCM registration token".into(),
                    ));
                }
                token
            }
        };
        Ok(Self { channel, token })
    }
}

/// The storage row contains a sensitive token. Never derive Debug or Serialize.
#[derive(Deserialize)]
pub struct Job {
    pub delivery_id: Uuid,
    pub message_id: Uuid,
    pub channel: Channel,
    pub token: String,
    pub title: String,
    pub body: String,
    pub accepted: bool,
    pub attempt: u32,
    pub lease: i64,
}

#[derive(Clone, Copy)]
pub enum Finish {
    Remove,
    InvalidTarget,
    Reschedule {
        accepted: bool,
        next_at: i64,
        accepted_deadline: Option<i64>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct StoreError;

#[allow(async_fn_in_trait)]
pub trait Outbox {
    /// Purge expired jobs and claim an eligible job with a fencing token.
    async fn claim(&self, now: i64, lease_until: i64) -> Result<Option<Job>, StoreError>;
    /// Update only the current lease. InvalidTarget also removes the exact target version.
    async fn finish(&self, job: &Job, finish: Finish) -> Result<(), StoreError>;
}

pub struct BatchReport {
    pub rejected: usize,
    pub authorization_blocked: usize,
}

pub async fn drain(
    store: &impl Outbox,
    transport: &impl Transport,
    now: impl Fn() -> i64,
    limit: usize,
) -> Result<BatchReport, StoreError> {
    let mut rejected = 0;
    let mut authorization_blocked = 0;
    for _ in 0..limit {
        let started_at = now();
        let Some(job) = store.claim(started_at, started_at + LEASE).await? else {
            break;
        };
        let result = transport.request(job.request()).await;
        let now = now();
        let decision = relay::decision(&job, result, now);
        let finish = match decision {
            Decision::Accepted => Finish::Reschedule {
                accepted: true,
                next_at: now + 30,
                accepted_deadline: Some(now + STATUS_WINDOW),
            },
            Decision::Poll => Finish::Reschedule {
                accepted: true,
                next_at: now + 60,
                accepted_deadline: None,
            },
            Decision::Retry(retry_after) | Decision::AuthorizationBlocked(retry_after) => {
                if matches!(decision, Decision::AuthorizationBlocked(_)) {
                    authorization_blocked += 1;
                }
                let backoff = (5_i64 << job.attempt.saturating_sub(1).min(6)).min(300);
                let jitter = i64::from(job.delivery_id.as_bytes()[15] % 5);
                Finish::Reschedule {
                    accepted: job.accepted,
                    next_at: now + (backoff + jitter).max(retry_after.min(STATUS_WINDOW)),
                    accepted_deadline: None,
                }
            }
            Decision::Done => Finish::Remove,
            Decision::InvalidTarget => Finish::InvalidTarget,
            Decision::Rejected => {
                rejected += 1;
                Finish::Remove
            }
        };
        store.finish(&job, finish).await?;
    }
    Ok(BatchReport {
        rejected,
        authorization_blocked,
    })
}
