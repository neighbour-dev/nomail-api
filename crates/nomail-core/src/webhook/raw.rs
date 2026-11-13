//! Complete decoded payload plus runtime-visible headers, excluding an explicit
//! case-insensitive blacklist. This is not a byte-for-byte HTTP request archive.
use super::{EventContent, HeaderLookup, platforms};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Common HTTP credentials. Platform-specific names are declared by each platform.
/// Exact names only: unknown/custom headers are retained unless explicitly listed.
/// Timestamp, nonce, delivery and version headers are not credentials.
pub const RAW_HEADER_BLACKLIST: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "api-key",
    "x-auth-token",
    "x-csrf-token",
    "x-xsrf-token",
    "cf-access-jwt-assertion",
    "x-forwarded-access-token",
];

const PLATFORM_RAW_HEADER_BLACKLISTS: &[&[&str]] = &[
    platforms::mailflare::RAW_HEADER_BLACKLIST,
    platforms::generic::RAW_HEADER_BLACKLIST,
    platforms::github::RAW_HEADER_BLACKLIST,
    platforms::gitea::RAW_HEADER_BLACKLIST,
    platforms::app_store_connect::RAW_HEADER_BLACKLIST,
    platforms::gitlab::RAW_HEADER_BLACKLIST,
    platforms::stripe::RAW_HEADER_BLACKLIST,
    platforms::slack::RAW_HEADER_BLACKLIST,
    platforms::feishu::RAW_HEADER_BLACKLIST,
    platforms::telegram::RAW_HEADER_BLACKLIST,
    platforms::shopify::RAW_HEADER_BLACKLIST,
];

pub fn is_raw_header_blocked(name: &str) -> bool {
    RAW_HEADER_BLACKLIST
        .iter()
        .chain(
            PLATFORM_RAW_HEADER_BLACKLISTS
                .iter()
                .flat_map(|headers| headers.iter()),
        )
        .any(|blocked| name.eq_ignore_ascii_case(blocked))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub enum RawHeaderEncoding {
    Base64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct RawHeader {
    pub name: String,
    pub value: String,
    /// Opaque non-UTF-8 bytes are retained without lossy decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<RawHeaderEncoding>,
}

impl RawHeader {
    pub fn from_bytes(name: impl Into<String>, value: &[u8]) -> Self {
        let (value, encoding) = match std::str::from_utf8(value) {
            Ok(value) => (value.to_owned(), None),
            Err(_) => (STANDARD.encode(value), Some(RawHeaderEncoding::Base64)),
        };
        Self {
            name: name.into(),
            value,
            encoding,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct RawWebhook {
    /// A list, not a map: duplicate names and their individual values survive.
    pub header: Vec<RawHeader>,
    pub payload: EventContent,
}

impl From<EventContent> for RawWebhook {
    fn from(payload: EventContent) -> Self {
        Self {
            header: Vec::new(),
            payload,
        }
    }
}

impl RawWebhook {
    /// Restore decoded raw data from its separate storage columns. No decryption,
    /// authentication, time-window check or endpoint configuration is involved.
    pub fn from_stored(
        header: Vec<RawHeader>,
        payload: &str,
        content_type: &str,
    ) -> Result<Self, super::WebhookError> {
        let kind = content_type.split(';').next().unwrap_or("").trim();
        let payload = match kind {
            "application/json" => {
                EventContent::Json(serde_json::from_str(payload).map_err(|_| {
                    super::WebhookError::InvalidPayload("invalid stored JSON payload")
                })?)
            }
            "text/plain" => EventContent::Text(payload.to_owned()),
            "application/x-www-form-urlencoded" => {
                EventContent::Form(serde_urlencoded::from_str(payload).map_err(|_| {
                    super::WebhookError::InvalidPayload("invalid stored form parameters")
                })?)
            }
            _ => {
                return Err(super::WebhookError::Unsupported(
                    "unsupported stored payload content type",
                ));
            }
        };
        Ok(Self { header, payload })
    }
    /// Only the decoded payload is stored in raw_payload, never this envelope.
    pub fn stored_payload(&self) -> String {
        match &self.payload {
            EventContent::Json(value) => {
                serde_json::to_string_pretty(value).expect("JSON value serializes")
            }
            EventContent::Text(value) => value.clone(),
            EventContent::Form(fields) => {
                serde_urlencoded::to_string(fields).expect("string form parameters serialize")
            }
        }
    }
    pub const fn payload_content_type(&self) -> &'static str {
        match self.payload {
            EventContent::Json(_) => "application/json",
            EventContent::Text(_) => "text/plain",
            EventContent::Form(_) => "application/x-www-form-urlencoded",
        }
    }
    pub(super) fn capture_headers(&mut self, source: &dyn HeaderLookup) {
        self.header = source
            .entries()
            .into_iter()
            .filter(|h| !is_raw_header_blocked(&h.name))
            .collect();
    }
    pub fn as_json(&self) -> Option<&Value> {
        self.payload.as_json()
    }
    pub fn as_text(&self) -> Option<&str> {
        self.payload.as_text()
    }
    /// Serialize the whole raw envelope, not just its decoded payload.
    pub fn to_readable_string(&self) -> String {
        serde_json::to_string_pretty(self)
            .expect("raw envelope contains only JSON-serializable data")
    }
    pub const fn media_type(&self) -> &'static str {
        "application/json"
    }
}

impl fmt::Display for RawWebhook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&serde_json::to_string(self).map_err(|_| fmt::Error)?)
    }
}

impl HeaderLookup for RawWebhook {
    fn values(&self, name: &str) -> Vec<&str> {
        self.header
            .iter()
            .filter(|h| {
                h.encoding.is_none() && h.value.is_ascii() && h.name.eq_ignore_ascii_case(name)
            })
            .map(|h| h.value.as_str())
            .collect()
    }
    fn entries(&self) -> Vec<RawHeader> {
        self.header.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webhook::{test_support::*, *};
    use serde_json::json;

    #[test]
    fn captures_every_non_blacklisted_header_without_collapsing_or_trimming() {
        let mut source = headers(&[
            ("X-Unknown", "one"),
            ("x-unknown", "two"),
            ("X-Empty", ""),
            ("X-Spaces", " value "),
            ("Authorization-Extra", "not a blacklisted name"),
            (
                "X-New-Platform-Secret",
                "unknown headers are retained by policy",
            ),
            ("webhook-id", "delivery-1"),
            ("webhook-timestamp", "123"),
            ("X-Lark-Request-Nonce", "nonce-1"),
        ]);
        let expected = source.entries();
        for name in RAW_HEADER_BLACKLIST.iter().chain(
            PLATFORM_RAW_HEADER_BLACKLISTS
                .iter()
                .flat_map(|headers| headers.iter()),
        ) {
            source.push((name.to_ascii_uppercase(), "private-header-value".into()));
            source.push(((*name).to_owned(), "private-header-value".into()));
        }
        let body = br#"{"title":"message","Authorization":"body data is not header data","extra":{"null":null}}"#;
        let parsed =
            event(receive_with(Provider::Generic, Authentication::None, &source, body).unwrap());
        assert_eq!(parsed.raw.header, expected);
        assert!(
            !serde_json::to_string(&parsed.raw.header)
                .unwrap()
                .contains("private-header-value")
        );
        assert_eq!(
            parsed.raw.as_json().unwrap()["Authorization"],
            "body data is not header data"
        );
        assert_eq!(parsed.raw.as_json().unwrap()["extra"]["null"], Value::Null);
        let serialized = serde_json::to_value(&parsed.raw).unwrap();
        assert_eq!(
            serialized["header"][2],
            json!({"name":"X-Empty","value":""})
        );
        assert_eq!(serialized["payload"]["format"], "json");
        let restored: RawWebhook = serde_json::from_value(serialized).unwrap();
        assert_eq!(restored, parsed.raw);
    }

    #[test]
    fn opaque_header_values_are_retained_without_lossy_decoding() {
        let text = RawHeader::from_bytes("X-Unknown", "中文".as_bytes());
        assert_eq!(text.value, "中文");
        assert_eq!(text.encoding, None);
        let binary = RawHeader::from_bytes("X-Binary", &[0xff, 0x80]);
        assert_eq!(binary.encoding, Some(RawHeaderEncoding::Base64));
        assert_eq!(STANDARD.decode(binary.value).unwrap(), [0xff, 0x80]);
    }

    #[test]
    fn receive_and_read_keep_non_ascii_raw_but_only_model_ascii_headers() {
        for (name, value) in [
            ("X-NoMail-Title", "中文"),
            ("X-Unknown", "é"),
            ("Authorization", "Bearer 中文"),
        ] {
            let source = headers(&[(name, value)]);
            let parsed = event(
                receive_with(
                    Provider::Generic,
                    Authentication::None,
                    &source,
                    "正文".as_bytes(),
                )
                .unwrap(),
            );
            assert_eq!(parse_raw(Provider::Generic, &parsed.raw).ok(), parsed.data);
            assert_eq!(
                parsed.raw.header,
                if is_raw_header_blocked(name) {
                    vec![]
                } else {
                    source.entries()
                }
            );
            assert_eq!(
                serde_json::to_value(parsed.data).unwrap()["event"]["header"],
                json!({})
            );
        }
        let source = headers(&[
            ("X-NoMail-Title", "ASCII title"),
            ("X-Unknown", ""),
            ("X-Unknown", "two"),
        ]);
        for body in ["正文", r#"{"title":"中文标题","content":"正文"}"#] {
            let parsed = event(
                receive_with(
                    Provider::Generic,
                    Authentication::None,
                    &source,
                    body.as_bytes(),
                )
                .unwrap(),
            );
            let restored = RawWebhook::from_stored(
                parsed.raw.header.clone(),
                &parsed.raw.stored_payload(),
                parsed.raw.payload_content_type(),
            )
            .unwrap();
            assert_eq!(parse_raw(Provider::Generic, &restored).ok(), parsed.data);
            assert_eq!(restored, parsed.raw);
        }
    }

    #[test]
    fn raw_envelope_retains_json_null_text_and_decoded_form_payload_formats() {
        for payload in [
            EventContent::Json(Value::Null),
            EventContent::Text(" text\n".into()),
            EventContent::Form(vec![
                ("command".into(), "/example".into()),
                ("text".into(), "".into()),
            ]),
        ] {
            let raw: RawWebhook = payload.clone().into();
            let restored: RawWebhook = serde_json::from_str(&raw.to_readable_string()).unwrap();
            assert_eq!(restored.payload, payload);
            assert!(restored.header.is_empty());
        }
    }
}
