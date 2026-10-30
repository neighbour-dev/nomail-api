//! Feishu credential policy, signature/decryption, control requests and inbox mapping.
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, IncomingMessage, PlatformData, PlatformEvent,
    ReceiveOutcome, WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeishuEvent {
    V2(FeishuV2Event),
    Legacy(FeishuLegacyEvent),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeishuV2Event {
    pub app_id: Option<String>,
    pub tenant_key: Option<String>,
    pub message: Option<FeishuMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeishuMessage {
    pub message_id: Option<String>,
    pub message_type: Option<String>,
    /// Feishu's original content string; text messages contain nested JSON here.
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeishuLegacyEvent {
    pub uuid: Option<String>,
}

pub(in crate::webhook) fn receive(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
) -> Result<ReceiveOutcome, WebhookError> {
    let (verification_token, encrypt_key) = match &config.authentication {
        Authentication::None => (None, None),
        Authentication::Feishu {
            verification_token,
            encrypt_key,
        } if (verification_token.is_some() || encrypt_key.is_some())
            && !verification_token.is_some_and(str::is_empty)
            && !encrypt_key.is_some_and(str::is_empty) =>
        {
            (*verification_token, *encrypt_key)
        }
        _ => return Err(invalid_auth()),
    };
    let signed = !request.header("x-lark-signature").is_empty()
        || !request.header("x-feishu-signature").is_empty();
    if let Some(key) = encrypt_key
        && signed
    {
        verify_signature(config, request, key)?;
    }
    // Unsigned encrypted URL verification must be decoded before its token can
    // be checked. Do not expose padding/JSON failures as a decryption oracle.
    let payload = decode(request.body, encrypt_key).map_err(|error| {
        if encrypt_key.is_some() && !signed {
            WebhookError::Unauthorized
        } else {
            error
        }
    })?;
    let is_challenge = text(&payload, &["type"]) == "url_verification"
        || (payload.get("challenge").is_some() && payload.get("event").is_none());
    if is_challenge {
        if let Some(token) = verification_token {
            crypto::equal(token.as_bytes(), text(&payload, &["token"]).as_bytes())?;
        } else if !matches!(config.authentication, Authentication::None) {
            return Err(WebhookError::InvalidConfiguration(
                "Feishu URL verification requires verification token",
            ));
        }
        return challenge(&payload);
    }
    if encrypt_key.is_some() {
        if !signed {
            return Err(WebhookError::Unauthorized);
        }
    } else if let Some(token) = verification_token {
        let actual = first(&[
            text(&payload, &["token"]),
            text(&payload, &["header", "token"]),
        ]);
        crypto::equal(token.as_bytes(), actual.as_bytes())?;
    }
    let schema = text(&payload, &["schema"]);
    let event_type = required(
        first(&[
            text(&payload, &["header", "event_type"]),
            text(&payload, &["type"]),
        ]),
        "missing Feishu event type",
    )?;
    let id = first(&[
        text(&payload, &["header", "event_id"]),
        text(&payload, &["uuid"]),
    ]);
    let event = match schema.as_str() {
        "2.0" => FeishuEvent::V2(FeishuV2Event {
            app_id: optional(text(&payload, &["header", "app_id"])),
            tenant_key: optional(text(&payload, &["header", "tenant_key"])),
            message: payload["event"]
                .get("message")
                .filter(|value| value.is_object())
                .map(|value| FeishuMessage {
                    message_id: optional(text(value, &["message_id"])),
                    message_type: optional(text(value, &["message_type"])),
                    content: optional(text(value, &["content"])),
                }),
        }),
        "" => FeishuEvent::Legacy(FeishuLegacyEvent {
            uuid: optional(text(&payload, &["uuid"])),
        }),
        _ => return Err(WebhookError::Unsupported("unsupported Feishu schema")),
    };
    Ok(PlatformEvent {
        metadata: metadata(event_type, optional(schema), "feishu:event", id),
        data: PlatformData::Feishu(event),
        raw: EventContent::Json(payload),
    }
    .into())
}

fn decode(body: &[u8], encrypt_key: Option<&str>) -> Result<Value, WebhookError> {
    let outer = object(body)?;
    if let Some(encrypted) = outer.get("encrypt") {
        let key = encrypt_key.ok_or(WebhookError::Unauthorized)?;
        let encrypted = encrypted
            .as_str()
            .ok_or(WebhookError::InvalidPayload("invalid encrypted payload"))?;
        let buffer = STANDARD
            .decode(encrypted)
            .map_err(|_| WebhookError::InvalidPayload("invalid encrypted payload"))?;
        if buffer.len() < 32 {
            return Err(WebhookError::InvalidPayload("invalid encrypted payload"));
        }
        let key = Sha256::digest(key.as_bytes());
        let plaintext = cbc::Decryptor::<aes::Aes256>::new_from_slices(&key, &buffer[..16])
            .map_err(|_| WebhookError::InvalidPayload("invalid encrypted payload"))?
            .decrypt_padded_vec_mut::<Pkcs7>(&buffer[16..])
            .map_err(|_| WebhookError::InvalidPayload("invalid encrypted payload"))?;
        object(&plaintext)
    } else {
        Ok(outer)
    }
}

fn verify_signature(
    config: &WebhookConfig<'_>,
    request: &WebhookRequest<'_>,
    key: &str,
) -> Result<(), WebhookError> {
    let time = first_header(
        request,
        "x-lark-request-timestamp",
        "x-feishu-request-timestamp",
    );
    let nonce = first_header(request, "x-lark-request-nonce", "x-feishu-request-nonce");
    let signature = first_header(request, "x-lark-signature", "x-feishu-signature");
    if nonce.is_empty() {
        return Err(WebhookError::Unauthorized);
    }
    crypto::timestamp(config, time)?;
    let mut hash = Sha256::new();
    hash.update(time);
    hash.update(nonce);
    hash.update(key);
    hash.update(request.body);
    crypto::equal(&hash.finalize(), &crypto::hex_signature(signature)?)
}

fn first_header<'a>(request: &'a WebhookRequest<'_>, name: &str, alternate: &str) -> &'a str {
    let value = request.header(name);
    if value.is_empty() {
        request.header(alternate)
    } else {
        value
    }
}

impl FeishuEvent {
    pub fn to_message(&self, metadata: &EventMetadata, raw: &EventContent) -> IncomingMessage {
        let p = raw.as_json().unwrap_or(&Value::Null);
        let message_fields = match self {
            Self::V2(event) => event.message.as_ref(),
            Self::Legacy(_) => None,
        };
        let raw_content = message_fields
            .and_then(|message| message.content.clone())
            .unwrap_or_else(|| text(p, &["event", "message", "content"]));
        let message_type = message_fields
            .and_then(|message| message.message_type.clone())
            .unwrap_or_else(|| text(p, &["event", "message", "message_type"]));
        let content = if message_type == "text" {
            serde_json::from_str::<Value>(&raw_content)
                .ok()
                .map(|value| text(&value, &["text"]))
                .filter(|value| !value.is_empty())
                .unwrap_or(raw_content)
        } else {
            raw_content
        };
        message(
            first(&[text(p, &["title"]), metadata.event_type.clone()]),
            first(&[
                text(p, &["content"]),
                content,
                text(p, &["event", "content"]),
                p["event"].to_string(),
            ]),
            first(&[text(p, &["source"]), "feishu".into()]),
            &text(p, &["level"]),
            text(p, &["url"]),
        )
    }
}
