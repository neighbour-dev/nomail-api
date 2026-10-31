//! NoMail inbox rules. Platform parsers do not assign receipt time or user state.
use super::{Authentication, MessageContent, Provider, WebhookError};

pub fn configured_provider(value: &str) -> Result<Provider, WebhookError> {
    match value.parse()? {
        provider @ (Provider::Generic | Provider::Github | Provider::Feishu) => Ok(provider),
        _ => Err(WebhookError::Unsupported("unsupported provider")),
    }
}

/// Current community API has one secret field. Never downgrade authentication
/// based on request headers. A future credential API can expose separate keys.
pub fn authentication(provider: Provider, secret: &str) -> Authentication<'_> {
    if secret.is_empty() {
        Authentication::None
    } else if provider == Provider::Feishu {
        Authentication::Feishu {
            verification_token: Some(secret),
            encrypt_key: Some(secret),
        }
    } else {
        Authentication::HmacSha256(secret)
    }
}

/// Modify common fields in-place. Never discard or rewrite platform data/raw.
pub fn apply_inbox_rules(
    message: &mut MessageContent,
    configured_source: Option<&str>,
    provider: Provider,
) {
    message.title = message.title.trim().chars().take(255).collect();
    if message.title.is_empty() {
        message.title = "Webhook message".into();
    }
    message.content = message.content.trim().into();
    let source = message
        .source
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or(configured_source.filter(|s| !s.trim().is_empty()))
        .unwrap_or(provider.as_str());
    message.source = Some(source.trim().chars().take(100).collect());
    message.url = message
        .url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webhook::{MessageLevel, ReceiveOutcome, WebhookConfig, WebhookRequest, receive};

    #[test]
    fn supported_providers_are_a_community_policy() {
        assert_eq!(configured_provider(" GitHub ").unwrap(), Provider::Github);
        assert_eq!(configured_provider("feishu").unwrap(), Provider::Feishu);
        assert_eq!(configured_provider("generic").unwrap(), Provider::Generic);
        assert!(configured_provider("stripe").is_err());
    }

    #[test]
    fn rules_only_change_common_fields_not_platform_content() {
        let headers = vec![];
        let ReceiveOutcome::Event(event) = receive(
            &WebhookConfig::new(Provider::Generic, Authentication::None, 1),
            &WebhookRequest { headers: &headers, body: br#"{"title":" Alert ","content":" body ","level":"urgent","custom":{"keep":true}}"# },
        ).unwrap() else { panic!() };
        let original = event.clone();
        let mut content = event.to_message_content();
        apply_inbox_rules(&mut content, Some(" monitor "), event.provider());
        assert_eq!(content.title, "Alert");
        assert_eq!(content.content, "body");
        assert_eq!(content.source.as_deref(), Some("monitor"));
        assert_eq!(content.level, MessageLevel::Urgent);
        assert_eq!(event, original);
    }

    #[test]
    fn priorities_have_an_explicit_order() {
        let levels = [
            MessageLevel::Low,
            MessageLevel::Normal,
            MessageLevel::High,
            MessageLevel::Urgent,
        ];
        for (rank, level) in levels.iter().enumerate() {
            assert_eq!(level.priority_rank(), rank as u8);
            for (threshold_rank, threshold) in levels.iter().enumerate() {
                assert_eq!(level.is_at_least(*threshold), rank >= threshold_rank);
            }
        }
    }
}
