//! Each platform owns its event types, authentication, decoding and message mapping.
pub mod feishu;
pub mod generic;
pub mod github;
pub mod gitlab;
pub mod shopify;
pub mod slack;
pub mod stripe;
pub mod telegram;

pub use feishu::FeishuEvent;
pub use generic::{GenericEvent, GenericJsonEvent, GenericTextEvent};
pub use github::GithubEvent;
pub use gitlab::GitlabEvent;
pub use shopify::ShopifyEvent;
pub use slack::SlackEvent;
pub use stripe::StripeEvent;
pub use telegram::TelegramEvent;

use super::{EventContent, EventMetadata, IncomingMessage, Provider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "event", rename_all = "lowercase")]
pub enum PlatformData {
    Generic(GenericEvent),
    Github(GithubEvent),
    Gitlab(GitlabEvent),
    Stripe(StripeEvent),
    Slack(SlackEvent),
    Feishu(FeishuEvent),
    Telegram(TelegramEvent),
    Shopify(ShopifyEvent),
}

/// A received business event: common processing information, platform semantics,
/// and complete decoded content. It contains neither HTTP headers nor wire bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformEvent {
    pub metadata: EventMetadata,
    pub data: PlatformData,
    pub raw: EventContent,
}

impl PlatformEvent {
    pub fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    pub const fn provider(&self) -> Provider {
        match &self.data {
            PlatformData::Generic(_) => Provider::Generic,
            PlatformData::Github(_) => Provider::Github,
            PlatformData::Gitlab(_) => Provider::Gitlab,
            PlatformData::Stripe(_) => Provider::Stripe,
            PlatformData::Slack(_) => Provider::Slack,
            PlatformData::Feishu(_) => Provider::Feishu,
            PlatformData::Telegram(_) => Provider::Telegram,
            PlatformData::Shopify(_) => Provider::Shopify,
        }
    }

    /// Apply built-in inbox mappings without network or storage access.
    /// Database limits, retention, IDs and tenant ownership are not mapping concerns.
    pub fn to_message(&self) -> IncomingMessage {
        match &self.data {
            PlatformData::Generic(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Github(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Gitlab(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Stripe(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Slack(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Feishu(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Telegram(event) => event.to_message(&self.metadata, &self.raw),
            PlatformData::Shopify(event) => event.to_message(&self.metadata, &self.raw),
        }
    }
}
