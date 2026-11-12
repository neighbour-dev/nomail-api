//! Native payload modeling, separate from authentication and content mapping.
use super::{
    PlatformData, app_store_connect, feishu, generic, gitea, github, gitlab, native::NativeValue,
    shopify, slack, stripe, telegram,
};
use crate::webhook::{EventContent, Provider, WebhookError, WebhookRequest};

pub(super) fn parse_payload(
    provider: Provider,
    raw: &EventContent,
    request: &WebhookRequest<'_>,
) -> Result<PlatformData, WebhookError> {
    let invalid = || WebhookError::InvalidPayload("invalid decoded platform payload");
    let form_json;
    let json = match raw {
        EventContent::Json(value) => Some(value),
        EventContent::Form(fields) => {
            let mut object = serde_json::Map::new();
            for (key, value) in fields {
                if object
                    .insert(key.clone(), serde_json::Value::String(value.clone()))
                    .is_some()
                {
                    return Err(WebhookError::InvalidPayload(
                        "duplicate stored form parameter",
                    ));
                }
            }
            form_json = serde_json::Value::Object(object);
            Some(&form_json)
        }
        EventContent::Text(_) => None,
    };
    macro_rules! restore {
        ($module:ident, $variant:ident, $webhook:ident, $header:ident, $payload:ident) => {
            PlatformData::$variant(Box::new($module::$webhook {
                header: $module::$header::from_request(request),
                payload: <$module::$payload as NativeValue>::read(json.ok_or_else(invalid)?)
                    .ok_or_else(invalid)?,
            }))
        };
    }
    Ok(match provider {
        Provider::Generic => {
            let payload = match raw {
                EventContent::Text(text) => generic::GenericWebhookPayload::Text(text.clone()),
                EventContent::Json(value) if value.is_null() => {
                    generic::GenericWebhookPayload::Null
                }
                _ => generic::GenericWebhookPayload::Json(Box::new(
                    generic::GenericJsonPayload::read(json.ok_or_else(invalid)?)
                        .ok_or_else(invalid)?,
                )),
            };
            PlatformData::Generic(Box::new(generic::GenericWebhook {
                header: generic::GenericWebhookHeader::from_request(request),
                payload,
            }))
        }
        Provider::Github => restore!(
            github,
            Github,
            GithubWebhook,
            GithubWebhookHeader,
            GithubWebhookPayload
        ),
        Provider::Gitea => restore!(
            gitea,
            Gitea,
            GiteaWebhook,
            GiteaWebhookHeader,
            GiteaWebhookPayload
        ),
        Provider::AppStoreConnect => restore!(
            app_store_connect,
            AppStoreConnect,
            AppStoreConnectWebhook,
            AppStoreConnectWebhookHeader,
            AppStoreConnectWebhookPayload
        ),
        Provider::Gitlab => restore!(
            gitlab,
            Gitlab,
            GitlabWebhook,
            GitlabWebhookHeader,
            GitlabWebhookPayload
        ),
        Provider::Stripe => restore!(
            stripe,
            Stripe,
            StripeWebhook,
            StripeWebhookHeader,
            StripeWebhookPayload
        ),
        Provider::Slack => restore!(
            slack,
            Slack,
            SlackWebhook,
            SlackWebhookHeader,
            SlackWebhookPayload
        ),
        Provider::Feishu => restore!(
            feishu,
            Feishu,
            FeishuWebhook,
            FeishuWebhookHeader,
            FeishuWebhookPayload
        ),
        Provider::Telegram => restore!(
            telegram,
            Telegram,
            TelegramWebhook,
            TelegramWebhookHeader,
            TelegramWebhookPayload
        ),
        Provider::Shopify => restore!(
            shopify,
            Shopify,
            ShopifyWebhook,
            ShopifyWebhookHeader,
            ShopifyWebhookPayload
        ),
    })
}
