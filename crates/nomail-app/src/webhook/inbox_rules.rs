//! NoMail inbox rules. Platform parsers do not assign receipt time or user state.
use super::{MessageContent, Provider};

/// Modify common fields in-place. Never discard or rewrite platform data/raw.
pub fn apply_inbox_rules(message: &mut MessageContent, _provider: Provider) {
    message.title = message.title.trim().chars().take(255).collect();
    if message.title.is_empty() {
        message.title = "Webhook message".into();
    }
    message.summary = crate::pagination::summarize(&message.summary);
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
    use crate::webhook::{
        Authentication, MessageLevel, ReceiveOutcome, WebhookConfig, WebhookRequest, receive,
    };

    #[test]
    fn rules_only_change_common_fields_not_platform_content() {
        let headers = vec![];
        let ReceiveOutcome::Event(event) = receive(
            &WebhookConfig::new(Provider::Generic, Authentication::None, 1),
            &WebhookRequest { headers: &headers, body: br#"{"title":" Alert ","content":" body ","level":"urgent","custom":{"keep":true}}"# },
        ).unwrap() else { panic!() };
        let original = event.clone();
        let mut content = event.to_message_content();
        apply_inbox_rules(&mut content, event.provider());
        assert_eq!(content.title, "Alert");
        assert_eq!(content.summary, "body");
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
