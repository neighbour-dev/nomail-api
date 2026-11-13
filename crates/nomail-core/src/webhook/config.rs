//! Owned, platform-discriminated protocol configuration. No product or storage policy.
pub use super::platforms::{
    app_store_connect::config::AppStoreConnectCredentials, feishu::config::FeishuCredentials,
    generic::config::GenericCredentials, gitea::config::GiteaCredentials,
    github::config::GithubCredentials, gitlab::config::GitlabCredentials,
    mailflare::config::MailflareCredentials, shopify::config::ShopifyCredentials,
    slack::config::SlackCredentials, stripe::config::StripeCredentials,
    telegram::config::TelegramCredentials,
};
use super::{Provider, WebhookConfig, WebhookError};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};

/// Contains secrets: never implement Debug or use directly as an API response.
/// Serialize is for the caller's encrypted persistence, not public presentation.
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "Self",
    tag = "provider",
    content = "credentials",
    rename_all = "lowercase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum PlatformConfig {
    Mailflare(#[serde(deserialize_with = "object_only")] MailflareCredentials),
    Generic(#[serde(deserialize_with = "object_only")] GenericCredentials),
    Github(#[serde(deserialize_with = "object_only")] GithubCredentials),
    Gitea(#[serde(deserialize_with = "object_only")] GiteaCredentials),
    AppStoreConnect(#[serde(deserialize_with = "object_only")] AppStoreConnectCredentials),
    Gitlab(#[serde(deserialize_with = "object_only")] GitlabCredentials),
    Stripe(#[serde(deserialize_with = "object_only")] StripeCredentials),
    Slack(#[serde(deserialize_with = "object_only")] SlackCredentials),
    Feishu(#[serde(deserialize_with = "object_only")] FeishuCredentials),
    Telegram(#[serde(deserialize_with = "object_only")] TelegramCredentials),
    Shopify(#[serde(deserialize_with = "object_only")] ShopifyCredentials),
}

impl Serialize for PlatformConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for PlatformConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ConfigVisitor;
        impl<'de> Visitor<'de> for ConfigVisitor {
            type Value = PlatformConfig;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a platform configuration object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                PlatformConfig::deserialize(MapAccessDeserializer::new(map))
            }
        }
        deserializer.deserialize_map(ConfigVisitor)
    }
}

// Serde structs otherwise also accept positional arrays. Preserve streaming map
// parsing (including duplicate-field rejection) without buffering into Value.
fn object_only<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    struct ObjectVisitor<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a credentials object")
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum AuthenticationMode {
    None,
    HmacSha256,
    GitlabSigningToken,
    GitlabSecretToken,
    StripeSigningSecrets,
    SlackSigningSecret,
    FeishuToken,
    FeishuEncrypted,
    TelegramSecretToken,
    ShopifyClientSecret,
}

/// Safe public projection. The mode also indicates which credentials are configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct PlatformSummary {
    pub provider: Provider,
    pub authentication: AuthenticationMode,
}

pub(super) fn secret(value: &str) -> Result<(), WebhookError> {
    if value.trim().is_empty() || value.chars().count() > 500 {
        Err(WebhookError::InvalidConfiguration(
            "credential must contain 1 to 500 nonblank characters",
        ))
    } else {
        Ok(())
    }
}

impl PlatformConfig {
    pub const fn provider(&self) -> Provider {
        match self {
            Self::Mailflare(_) => Provider::Mailflare,
            Self::Generic(_) => Provider::Generic,
            Self::Github(_) => Provider::Github,
            Self::Gitea(_) => Provider::Gitea,
            Self::AppStoreConnect(_) => Provider::AppStoreConnect,
            Self::Gitlab(_) => Provider::Gitlab,
            Self::Stripe(_) => Provider::Stripe,
            Self::Slack(_) => Provider::Slack,
            Self::Feishu(_) => Provider::Feishu,
            Self::Telegram(_) => Provider::Telegram,
            Self::Shopify(_) => Provider::Shopify,
        }
    }

    /// Pure validation; errors never echo credential values.
    pub fn validate(&self) -> Result<(), WebhookError> {
        match self {
            Self::Mailflare(c) => c.validate(),
            Self::Generic(c) => c.validate(),
            Self::Github(c) => c.validate(),
            Self::Gitea(c) => c.validate(),
            Self::AppStoreConnect(c) => c.validate(),
            Self::Gitlab(c) => c.validate(),
            Self::Stripe(c) => c.validate(),
            Self::Slack(c) => c.validate(),
            Self::Feishu(c) => c.validate(),
            Self::Telegram(c) => c.validate(),
            Self::Shopify(c) => c.validate(),
        }
    }

    pub fn summary(&self) -> PlatformSummary {
        let authentication = match self {
            Self::Mailflare(c) => c.mode(),
            Self::Generic(c) => c.mode(),
            Self::Github(c) => c.mode(),
            Self::Gitea(c) => c.mode(),
            Self::AppStoreConnect(c) => c.mode(),
            Self::Gitlab(c) => c.mode(),
            Self::Stripe(c) => c.mode(),
            Self::Slack(c) => c.mode(),
            Self::Feishu(c) => c.mode(),
            Self::Telegram(c) => c.mode(),
            Self::Shopify(c) => c.mode(),
        };
        PlatformSummary {
            provider: self.provider(),
            authentication,
        }
    }

    /// Provider and authentication are derived together, never independently selected.
    pub fn receiving_config(&self, now: i64) -> Result<WebhookConfig<'_>, WebhookError> {
        self.validate()?;
        let authentication = match self {
            Self::Mailflare(c) => c.authentication(),
            Self::Generic(c) => c.authentication(),
            Self::Github(c) => c.authentication(),
            Self::Gitea(c) => c.authentication(),
            Self::AppStoreConnect(c) => c.authentication(),
            Self::Gitlab(c) => c.authentication(),
            Self::Stripe(c) => c.authentication(),
            Self::Slack(c) => c.authentication(),
            Self::Feishu(c) => c.authentication(),
            Self::Telegram(c) => c.authentication(),
            Self::Shopify(c) => c.authentication(),
        };
        Ok(WebhookConfig::new(self.provider(), authentication, now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn all_platforms_round_trip_validate_and_redact() {
        for value in [
            json!({"provider":"mailflare","credentials":{"secret":"private"}}),
            json!({"provider":"generic","credentials":{"type":"none"}}),
            json!({"provider":"generic","credentials":{"type":"hmacSha256","secret":"private"}}),
            json!({"provider":"github","credentials":{"secret":"private"}}),
            json!({"provider":"gitea","credentials":{"secret":"private"}}),
            json!({"provider":"appstoreconnect","credentials":{"secret":"private"}}),
            json!({"provider":"gitlab","credentials":{"type":"signingToken","signingToken":"whsec_cHJpdmF0ZQ=="}}),
            json!({"provider":"gitlab","credentials":{"type":"secretToken","secretToken":"private"}}),
            json!({"provider":"stripe","credentials":{"signingSecrets":["whsec_private"]}}),
            json!({"provider":"slack","credentials":{"signingSecret":"private"}}),
            json!({"provider":"feishu","credentials":{"verificationToken":"private"}}),
            json!({"provider":"feishu","credentials":{"verificationToken":"private","encryptKey":"different"}}),
            json!({"provider":"telegram","credentials":{"secretToken":"private"}}),
            json!({"provider":"shopify","credentials":{"clientSecret":"private"}}),
        ] {
            let config: PlatformConfig = serde_json::from_value(value.clone()).unwrap();
            assert!(config.validate().is_ok());
            assert_eq!(
                config.receiving_config(1000).unwrap().provider,
                config.provider()
            );
            assert_eq!(serde_json::to_value(&config).unwrap(), value);
            assert!(
                !serde_json::to_string(&config.summary())
                    .unwrap()
                    .contains("private")
            );
        }
    }

    #[test]
    fn unknown_fields_types_modes_and_cross_platform_credentials_are_rejected() {
        for value in [
            json!({"provider":"mailflare","credentials":{}}),
            json!({"provider":"mailflare","credentials":{"secret":1}}),
            json!({"provider":"mailflare","credentials":{"secret":"ok","type":"none"}}),
            json!({"provider":"mailflare","credentials":["secret"]}),
            json!({"provider":"unknown","credentials":{}}),
            json!({"provider":"appstoreconnect","credentials":{}}),
            json!({"provider":"appstoreconnect","credentials":{"secret":1}}),
            json!({"provider":"appstoreconnect","credentials":{"secret":"ok","privateKey":"wrong"}}),
            json!({"provider":"appstoreconnect","credentials":{"secret":"ok","type":"none"}}),
            json!({"provider":"appstoreconnect","credentials":["secret"]}),
            json!({"provider":"gitea","credentials":{}}),
            json!({"provider":"gitea","credentials":{"secret":1}}),
            json!({"provider":"gitea","credentials":{"secret":"ok","type":"none"}}),
            json!({"provider":"gitea","credentials":{"secretToken":"wrong"}}),
            json!({"provider":"gitea","credentials":["secret"]}),
            json!({"provider":"github","credentials":{"secret":"ok"},"extra":true}),
            json!({"provider":"github","credentials":{"secret":"ok","encryptKey":"wrong"}}),
            json!({"provider":"github","credentials":{"secretToken":"wrong"}}),
            json!({"provider":"github","credentials":{"secret":1}}),
            json!({"provider":"github","credentials":{}}),
            json!({"provider":"github","credentials":["secret"]}),
            json!(["github", {"secret":"secret"}]),
            json!({"provider":"generic","credentials":{}}),
            json!({"provider":"generic","credentials":{"type":"none","secret":"wrong"}}),
            json!({"provider":"gitlab","credentials":{"type":"secretToken","secretToken":"ok","signingToken":"wrong"}}),
        ] {
            assert!(
                serde_json::from_value::<PlatformConfig>(value.clone()).is_err(),
                "unexpectedly accepted: {value}"
            );
        }
    }

    #[test]
    fn semantic_validation_rejects_empty_or_malformed_credentials() {
        for value in [
            json!({"provider":"mailflare","credentials":{"secret":" "}}),
            json!({"provider":"github","credentials":{"secret":" "}}),
            json!({"provider":"gitea","credentials":{"secret":" "}}),
            json!({"provider":"appstoreconnect","credentials":{"secret":" "}}),
            json!({"provider":"feishu","credentials":{"verificationToken":"token","encryptKey":""}}),
            json!({"provider":"telegram","credentials":{"secretToken":"bot:token"}}),
            json!({"provider":"gitlab","credentials":{"type":"signingToken","signingToken":"whsec_!"}}),
            json!({"provider":"stripe","credentials":{"signingSecrets":[]}}),
        ] {
            let config: PlatformConfig = serde_json::from_value(value).unwrap();
            assert!(config.validate().is_err());
            assert!(config.receiving_config(1000).is_err());
        }
    }
}
