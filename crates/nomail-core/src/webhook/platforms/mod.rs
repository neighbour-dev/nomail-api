//! Each platform owns its event types, authentication, decoding and message mapping.
// A platform branch binds its allowlisted delivery headers to its native payload.
// It is not an HTTP archive: credentials and signatures never enter this model.
macro_rules! webhook {
    ($name:ident, $header:ident, $payload:ty) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            pub header: $header,
            pub payload: $payload,
        }
    };
}

macro_rules! webhook_header {
    ($name:ident { $($field:ident = $wire:literal),* $(,)? }) => {
        #[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
        pub struct $name {
            $(#[serde(default, skip_serializing_if = "Option::is_none")]
            pub $field: Option<String>,)*
        }
        impl $name {
            pub(in crate::webhook) fn from_request(_request: &crate::webhook::WebhookRequest<'_>) -> Self {
                Self { $($field: {
                    let value = _request.header($wire);
                    (!value.is_empty() && value.is_ascii()).then(|| value.to_owned())
                },)* }
            }
        }
    };
}

#[macro_use]
mod native;
mod parse;
pub use native::{JsonField, JsonFieldExt};
pub use parse::parse_raw;
pub mod app_store_connect;
pub mod feishu;
pub mod generic;
pub mod gitea;
pub mod github;
pub mod gitlab;
pub mod mailflare;
pub mod shopify;
pub mod slack;
pub mod stripe;
pub mod telegram;

pub use app_store_connect::AppStoreConnectWebhook;
pub use feishu::FeishuWebhook;
pub use generic::GenericWebhook;
pub use gitea::GiteaWebhook;
pub use github::GithubWebhook;
pub use gitlab::GitlabWebhook;
pub use mailflare::MailflareWebhook;
pub use shopify::ShopifyWebhook;
pub use slack::SlackWebhook;
pub use stripe::StripeWebhook;
pub use telegram::TelegramWebhook;

use super::{EventContent, EventMetadata, MessageContent, Provider, RawWebhook, WebhookRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "event", rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum PlatformData {
    Mailflare(Box<MailflareWebhook>),
    Generic(Box<GenericWebhook>),
    Github(Box<GithubWebhook>),
    Gitea(Box<GiteaWebhook>),
    AppStoreConnect(Box<AppStoreConnectWebhook>),
    Gitlab(Box<GitlabWebhook>),
    Stripe(Box<StripeWebhook>),
    Slack(Box<SlackWebhook>),
    Feishu(Box<FeishuWebhook>),
    Telegram(Box<TelegramWebhook>),
    Shopify(Box<ShopifyWebhook>),
}

/// A received business event: common processing information, platform semantics,
/// and complete decoded content. Typed delivery headers are allowlisted; raw
/// headers are captured separately using the shared blacklist, never wire bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct PlatformEvent {
    /// Trusted endpoint provider, independent of successful semantic modeling.
    pub provider: Provider,
    pub metadata: EventMetadata,
    pub data: Option<PlatformData>,
    pub raw: RawWebhook,
}

impl PlatformEvent {
    pub fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }

    pub const fn provider(&self) -> Provider {
        self.provider
    }

    /// Called only after authentication and transport decoding/decryption.
    /// Modeling failure must never discard an accepted, decoded delivery.
    pub(super) fn from_raw(
        provider: Provider,
        metadata: EventMetadata,
        payload: EventContent,
        request: &WebhookRequest<'_>,
    ) -> Self {
        let mut raw: RawWebhook = payload.into();
        raw.capture_headers(request.headers);
        let data = parse_raw(provider, &raw).ok();
        Self {
            provider,
            metadata,
            data,
            raw,
        }
    }

    /// Apply built-in inbox mappings without network or storage access.
    /// Only common business fields are mapped. This does NOT replace the platform
    /// data/raw, nor assign database identity, retention or NoMail user state.
    pub fn to_message_content(&self) -> MessageContent {
        let Some(data) = &self.data else {
            return super::support::message(
                format!("{} notification", self.provider.as_str()),
                String::new(),
                "normal",
                String::new(),
            );
        };
        let mut common = match data {
            PlatformData::Mailflare(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Generic(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Github(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Gitea(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::AppStoreConnect(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Gitlab(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Stripe(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Slack(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Feishu(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Telegram(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
            PlatformData::Shopify(event) => {
                event.to_message_content(&self.metadata, &self.raw.payload)
            }
        };
        if common.title.trim().is_empty() {
            common.title = format!("{} notification", self.provider.as_str());
        }
        common
    }
}
