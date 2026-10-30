//! Actix boundary for the runtime-independent webhook core.
use crate::error::ApiError;
use actix_web::http::header::HeaderMap;
use nomail_core::webhook::{
    self, Authentication, HeaderLookup, MessageLevel, Provider, ReceiveOutcome, WebhookConfig,
    WebhookRequest,
};

struct Headers<'a>(&'a HeaderMap);

impl HeaderLookup for Headers<'_> {
    fn values(&self, name: &str) -> Vec<&str> {
        self.0
            .get_all(name)
            .filter_map(|value| value.to_str().ok())
            .collect()
    }
}

/// Keep the existing HTTP API's provider allowlist until new endpoint
/// authentication/configuration fields are exposed by the server.
pub fn provider(value: &str) -> Result<&'static str, ApiError> {
    match value.parse::<Provider>()? {
        Provider::Generic => Ok("generic"),
        Provider::Github => Ok("github"),
        Provider::Feishu => Ok("feishu"),
        _ => Err(ApiError::bad("unsupported provider")),
    }
}

pub fn receive(
    provider: &str,
    headers: &HeaderMap,
    body: &[u8],
    secret: &str,
    now: i64,
) -> Result<ReceiveOutcome, ApiError> {
    let provider = provider.parse::<Provider>()?;
    let authentication = if secret.is_empty() {
        Authentication::None
    } else if provider == Provider::Feishu {
        // Legacy server has only one secret field. Do not select a weaker auth
        // mode based on attacker-controlled missing signature headers.
        // Separate Feishu credentials are supported by core's typed config.
        Authentication::Feishu {
            verification_token: Some(secret),
            encrypt_key: Some(secret),
        }
    } else {
        Authentication::HmacSha256(secret)
    };
    Ok(webhook::receive(
        &WebhookConfig::new(provider, authentication, now),
        &WebhookRequest {
            headers: &Headers(headers),
            body,
        },
    )?)
}

pub fn normalize_level(value: &str) -> &'static str {
    MessageLevel::from_label(value).as_str()
}

pub fn retention_days(level: &str) -> i64 {
    match level {
        "critical" => 30,
        "warning" => 7,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::{
        StatusCode,
        header::{HeaderName, HeaderValue},
    };
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    #[test]
    fn server_adapter_preserves_header_values_and_maps_message() {
        let body = br#"{"id":"event-1","title":"Alert","level":"critical"}"#;
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(b"secret").unwrap();
        mac.update(body);
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-nomail-signature"),
            HeaderValue::from_str(&signature).unwrap(),
        );
        headers.append(
            HeaderName::from_static("x-repeated"),
            HeaderValue::from_static("one"),
        );
        headers.append(
            HeaderName::from_static("x-repeated"),
            HeaderValue::from_static("two"),
        );
        assert_eq!(Headers(&headers).values("X-Repeated"), ["one", "two"]);
        let ReceiveOutcome::Event(event) =
            receive("generic", &headers, body, "secret", 1000).unwrap()
        else {
            panic!()
        };
        assert_eq!(event.to_message().title, "Alert");
        assert_eq!(event.to_message().level, MessageLevel::Critical);
        assert_eq!(
            receive("generic", &headers, b"tampered", "secret", 1000)
                .unwrap_err()
                .status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn feishu_legacy_config_challenge_and_no_signature_downgrade() {
        let body = br#"{"type":"url_verification","challenge":"value","token":"secret"}"#;
        assert!(matches!(
            receive("feishu", &HeaderMap::new(), body, "secret", 1000).unwrap(),
            ReceiveOutcome::Respond(_)
        ));
        let body = br#"{"type":"event","token":"secret","event":{}}"#;
        assert_eq!(
            receive("feishu", &HeaderMap::new(), body, "secret", 1000)
                .unwrap_err()
                .status,
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn server_provider_allowlist_and_retention_are_unchanged() {
        assert_eq!(provider(" GitHub ").unwrap(), "github");
        assert!(provider("stripe").is_err());
        assert_eq!(normalize_level("ERROR"), "critical");
        assert_eq!(retention_days("critical"), 30);
        assert_eq!(retention_days("warning"), 7);
        assert_eq!(retention_days("normal"), 3);
    }
}
