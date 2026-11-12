//! Authenticate and decode platform requests, then map events to inbox messages.
//!
//! Provider and authentication policy must come from trusted endpoint configuration,
//! never from request headers. Pass the original, unmodified request body.
mod config;
mod crypto;
mod error;
pub mod platforms;
mod raw;
mod support;
mod types;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use config::*;
pub use error::WebhookError;
pub use platforms::*;
pub use raw::{
    RAW_HEADER_BLACKLIST, RawHeader, RawHeaderEncoding, RawWebhook, is_raw_header_blocked,
};
pub use types::*;

/// Authenticate, decrypt/decode, and identify a platform event or control request.
/// No external API calls, persistence, HTTP responses or acknowledgements happen here.
pub fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    if config.now < 0 {
        return Err(support::invalid_auth());
    }
    match config.provider {
        Provider::Generic => platforms::generic::receive(config, request),
        Provider::Github => platforms::github::receive(config, request),
        Provider::Gitea => platforms::gitea::receive(config, request),
        Provider::AppStoreConnect => platforms::app_store_connect::receive(config, request),
        Provider::Gitlab => platforms::gitlab::receive(config, request),
        Provider::Stripe => platforms::stripe::receive(config, request),
        Provider::Slack => platforms::slack::receive(config, request),
        Provider::Feishu => platforms::feishu::receive(config, request),
        Provider::Telegram => platforms::telegram::receive(config, request),
        Provider::Shopify => platforms::shopify::receive(config, request),
    }
}
