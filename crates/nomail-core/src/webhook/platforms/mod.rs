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
use parse::parse_payload;
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

pub use app_store_connect::AppStoreConnectWebhook;
pub use feishu::FeishuWebhook;
pub use generic::GenericWebhook;
pub use gitea::GiteaWebhook;
pub use github::GithubWebhook;
pub use gitlab::GitlabWebhook;
pub use shopify::ShopifyWebhook;
pub use slack::SlackWebhook;
pub use stripe::StripeWebhook;
pub use telegram::TelegramWebhook;

use super::{EventContent, EventMetadata, MessageContent, Provider, WebhookError, WebhookRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "event", rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum PlatformData {
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
/// and complete decoded content. Typed delivery headers are allowlisted.
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

    /// Called only after authentication and transport decoding/decryption.
    pub(super) fn from_payload(
        provider: Provider,
        metadata: EventMetadata,
        payload: EventContent,
        request: &WebhookRequest<'_>,
    ) -> Result<Self, WebhookError> {
        let data = parse_payload(provider, &payload, request)?;
        Ok(Self {
            metadata,
            data,
            raw: payload,
        })
    }

    /// Apply built-in inbox mappings without network or storage access.
    /// Only common business fields are mapped. This does NOT replace the platform
    /// data/raw, nor assign database identity, retention or NoMail user state.
    pub fn to_message_content(&self) -> MessageContent {
        let data = &self.data;
        match data {
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
