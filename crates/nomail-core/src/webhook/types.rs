use super::{PlatformEvent, WebhookError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum Provider {
    Generic,
    Github,
    Gitea,
    AppStoreConnect,
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
            Self::Gitea => "gitea",
            Self::AppStoreConnect => "appstoreconnect",
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
            "gitea" => Ok(Self::Gitea),
            "appstoreconnect" => Ok(Self::AppStoreConnect),
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
    StripeSigningSecrets(&'a [String]),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct DeduplicationKey {
    /// Must additionally be scoped by tenant and webhook endpoint in storage.
    pub scope: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct EventMetadata {
    pub event_type: String,
    pub version: Option<String>,
    pub deduplication_key: Option<DeduplicationKey>,
}

/// Complete human-readable platform content after transport decoding/decryption,
/// before semantic modeling or message mapping. Not the bytes used for signatures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "format", content = "value", rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum EventContent {
    Json(Value),
    Text(String),
    Form(Vec<(String, String)>),
}

impl EventContent {
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    /// A readable representation, not a reconstruction of the HTTP request.
    pub fn to_readable_string(&self) -> String {
        match self {
            Self::Json(value) => format!("{value:#}"),
            _ => self.to_string(),
        }
    }

    /// Media type of to_readable_string(), not the inbound request's Content-Type.
    pub const fn media_type(&self) -> &'static str {
        match self {
            Self::Json(_) => "application/json",
            Self::Text(_) | Self::Form(_) => "text/plain",
        }
    }
}

impl fmt::Display for EventContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(value) => write!(f, "{value}"),
            Self::Text(value) => f.write_str(value),
            Self::Form(fields) => {
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index != 0 {
                        f.write_str("\n")?;
                    }
                    write!(f, "{key}: {value}")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveOutcome {
    Event(Box<PlatformEvent>),
    /// Control requests only; ordinary events must be durably accepted before ACK.
    Respond(ProtocolResponse),
    Ignored {
        reason: &'static str,
    },
}

impl From<PlatformEvent> for ReceiveOutcome {
    fn from(event: PlatformEvent) -> Self {
        Self::Event(Box::new(event))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum MessageLevel {
    Low,
    Normal,
    High,
    Urgent,
}

impl MessageLevel {
    /// Attention priority, independent of platform outcome and retention policy.
    pub const fn priority_rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Urgent => 3,
        }
    }

    pub const fn is_at_least(self, threshold: Self) -> bool {
        self.priority_rank() >= threshold.priority_rank()
    }
    pub fn from_label(value: &str) -> Self {
        value.parse().unwrap_or(Self::Normal)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

/// Portable platform content, without inbox state or product-specific defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageContent {
    pub title: String,
    pub content: String,
    pub source: Option<String>,
    pub level: MessageLevel,
    pub url: Option<String>,
}

impl FromStr for MessageLevel {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            _ => Err("level must be low, normal, high or urgent"),
        }
    }
}
