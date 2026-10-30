use super::PlatformEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Generic,
    Github,
    Gitlab,
    Stripe,
    Slack,
    Feishu,
    Telegram,
    Shopify,
}

impl Provider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Stripe => "stripe",
            Self::Slack => "slack",
            Self::Feishu => "feishu",
            Self::Telegram => "telegram",
            Self::Shopify => "shopify",
        }
    }
}

impl FromStr for Provider {
    type Err = WebhookError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "generic" => Ok(Self::Generic),
            "github" => Ok(Self::Github),
            "gitlab" => Ok(Self::Gitlab),
            "stripe" => Ok(Self::Stripe),
            "slack" => Ok(Self::Slack),
            "feishu" => Ok(Self::Feishu),
            "telegram" => Ok(Self::Telegram),
            "shopify" => Ok(Self::Shopify),
            _ => Err(WebhookError::Unsupported("unsupported provider")),
        }
    }
}

/// Lookup must be ASCII case-insensitive and preserve all values of repeated headers.
pub trait HeaderLookup {
    fn values(&self, name: &str) -> Vec<&str>;

    fn get(&self, name: &str) -> Option<&str> {
        self.values(name).into_iter().next()
    }
}

impl HeaderLookup for Vec<(String, String)> {
    fn values(&self, name: &str) -> Vec<&str> {
        self.iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
}

pub struct WebhookRequest<'a> {
    pub headers: &'a dyn HeaderLookup,
    pub body: &'a [u8],
}

impl WebhookRequest<'_> {
    pub(super) fn header(&self, name: &str) -> &str {
        self.headers.get(name).unwrap_or("")
    }
}

/// Explicit authentication modes prevent fallback to weaker authentication.
/// Secrets intentionally do not implement Debug or Serialize.
pub enum Authentication<'a> {
    /// Only choose this explicitly for public/unsigned endpoints.
    None,
    HmacSha256(&'a str),
    GitlabToken(&'a str),
    GitlabSigningToken(&'a str),
    StripeSigningSecret(&'a str),
    SlackSigningSecret(&'a str),
    Feishu {
        verification_token: Option<&'a str>,
        encrypt_key: Option<&'a str>,
    },
    TelegramSecretToken(&'a str),
    ShopifyClientSecret(&'a str),
}

pub struct WebhookConfig<'a> {
    pub provider: Provider,
    pub authentication: Authentication<'a>,
    /// Caller-supplied Unix seconds; core never accesses the system clock.
    pub now: i64,
    /// Accepted absolute timestamp skew for timestamped signatures. Default: 300s.
    pub timestamp_tolerance: u64,
}

impl<'a> WebhookConfig<'a> {
    pub fn new(provider: Provider, authentication: Authentication<'a>, now: i64) -> Self {
        Self {
            provider,
            authentication,
            now,
            timestamp_tolerance: 300,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookError {
    InvalidConfiguration(&'static str),
    Unauthorized,
    InvalidPayload(&'static str),
    Unsupported(&'static str),
}

impl fmt::Display for WebhookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration(message)
            | Self::InvalidPayload(message)
            | Self::Unsupported(message) => message,
            Self::Unauthorized => "unauthorized",
        })
    }
}

impl std::error::Error for WebhookError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeduplicationKey {
    /// Must additionally be scoped by tenant and webhook endpoint in storage.
    pub scope: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    pub event_type: String,
    pub version: Option<String>,
    pub deduplication_key: Option<DeduplicationKey>,
}

/// Platform data is retained, including fields unknown to current built-in mappings.
/// These are protocol envelopes, not claims to implement every upstream event schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformPayload {
    pub metadata: EventMetadata,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveOutcome {
    Event(PlatformEvent),
    /// Control requests only; ordinary events must be durably accepted before ACK.
    Respond(ProtocolResponse),
    Ignored {
        reason: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    Normal,
    Success,
    Warning,
    Critical,
}

impl MessageLevel {
    pub fn from_label(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "critical" | "alert" | "error" | "failed" | "failure" => Self::Critical,
            "warning" | "warn" => Self::Warning,
            "success" | "pass" | "passed" | "ok" => Self::Success,
            _ => Self::Normal,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub title: String,
    pub content: String,
    pub source: Option<String>,
    pub level: MessageLevel,
    pub url: Option<String>,
}
