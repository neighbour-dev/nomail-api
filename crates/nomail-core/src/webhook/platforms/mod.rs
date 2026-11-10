//! Each platform owns its event types, authentication, decoding and message mapping.
pub mod app_store_connect;
pub mod feishu;
pub mod generic;
pub mod gitea;
pub mod github;
pub mod gitlab;
pub mod shopify;
pub mod slack;
pub mod stripe;
pub mod telegram;

pub use app_store_connect::AppStoreConnectEvent;
pub use feishu::FeishuEvent;
pub use generic::{GenericEvent, GenericJsonEvent, GenericTextEvent};
pub use gitea::GiteaEvent;
pub use github::GithubEvent;
pub use gitlab::GitlabEvent;
pub use shopify::ShopifyEvent;
pub use slack::SlackEvent;
pub use stripe::StripeEvent;
pub use telegram::TelegramEvent;

use super::{EventContent, EventMetadata, MessageContent, Provider};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "event", rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum PlatformData {
    Generic(GenericEvent),
    Github(Box<GithubEvent>),
    Gitea(Box<GiteaEvent>),
    AppStoreConnect(Box<AppStoreConnectEvent>),
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
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
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
            PlatformData::Gitea(_) => Provider::Gitea,
            PlatformData::AppStoreConnect(_) => Provider::AppStoreConnect,
            PlatformData::Gitlab(_) => Provider::Gitlab,
            PlatformData::Stripe(_) => Provider::Stripe,
            PlatformData::Slack(_) => Provider::Slack,
            PlatformData::Feishu(_) => Provider::Feishu,
            PlatformData::Telegram(_) => Provider::Telegram,
            PlatformData::Shopify(_) => Provider::Shopify,
        }
    }

    /// Apply built-in inbox mappings without network or storage access.
    /// Only common business fields are mapped. This does NOT replace the platform
    /// data/raw, nor assign database identity, retention or NoMail user state.
    pub fn to_message_content(&self) -> MessageContent {
        match &self.data {
            PlatformData::Generic(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Github(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Gitea(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::AppStoreConnect(event) => {
                event.to_message_content(&self.metadata, &self.raw)
            }
            PlatformData::Gitlab(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Stripe(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Slack(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Feishu(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Telegram(event) => event.to_message_content(&self.metadata, &self.raw),
            PlatformData::Shopify(event) => event.to_message_content(&self.metadata, &self.raw),
        }
    }
}
