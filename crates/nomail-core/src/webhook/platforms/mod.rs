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
pub use generic::{GenericDefaults, GenericEvent, GenericPayload};
pub use github::GithubEvent;
pub use gitlab::GitlabEvent;
pub use shopify::ShopifyEvent;
pub use slack::SlackEvent;
pub use stripe::StripeEvent;
pub use telegram::TelegramEvent;

use super::{EventMetadata, IncomingMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "event", rename_all = "lowercase")]
pub enum PlatformEvent {
    Generic(GenericEvent),
    Github(GithubEvent),
    Gitlab(GitlabEvent),
    Stripe(StripeEvent),
    Slack(SlackEvent),
    Feishu(FeishuEvent),
    Telegram(TelegramEvent),
    Shopify(ShopifyEvent),
}

impl PlatformEvent {
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            Self::Generic(event) => event.metadata(),
            Self::Github(event) => event.metadata(),
            Self::Gitlab(event) => event.metadata(),
            Self::Stripe(event) => event.metadata(),
            Self::Slack(event) => event.metadata(),
            Self::Feishu(event) => event.metadata(),
            Self::Telegram(event) => event.metadata(),
            Self::Shopify(event) => event.metadata(),
        }
    }

    /// Apply built-in inbox mappings without network or storage access.
    /// Database limits, retention, IDs and tenant ownership are not mapping concerns.
    pub fn to_message(&self) -> IncomingMessage {
        match self {
            Self::Generic(event) => event.to_message(),
            Self::Github(event) => event.to_message(),
            Self::Gitlab(event) => event.to_message(),
            Self::Stripe(event) => event.to_message(),
            Self::Slack(event) => event.to_message(),
            Self::Feishu(event) => event.to_message(),
            Self::Telegram(event) => event.to_message(),
            Self::Shopify(event) => event.to_message(),
        }
    }
}
