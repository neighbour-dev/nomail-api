//! Feishu credential policy, signature/decryption, control requests and inbox mapping.
pub mod config;
pub(in crate::webhook) const RAW_HEADER_BLACKLIST: &[&str] =
    &["x-lark-signature", "x-feishu-signature"];
#[cfg(test)]
mod tests;
use crate::webhook::{
    Authentication, EventContent, EventMetadata, MessageContent, PlatformEvent, ReceiveOutcome,
    WebhookConfig, WebhookError, WebhookRequest, crypto, support::*,
};
use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha2::{Digest, Sha256};

webhook_header!(FeishuWebhookHeader {});
webhook!(FeishuWebhook, FeishuWebhookHeader, FeishuWebhookPayload);
native_model!(FeishuWebhookPayload {
    schema: String = "schema",
    header: FeishuEventHeader = "header",
    event: FeishuEventPayload = "event",
    uuid: String = "uuid",
    event_type: String = "type",
    ts: Value = "ts",
});
// This header is part of the native v2 JSON payload, not an HTTP header.
// Its verification token is deliberately not included in the persisted model.
native_model!(FeishuEventHeader {
    event_id: String = "event_id",
    event_type: String = "event_type",
    create_time: String = "create_time",
    app_id: String = "app_id",
    tenant_key: String = "tenant_key",
});
native_model!(FeishuEventPayload {
    message: FeishuMessage = "message",
    sender: FeishuSender = "sender",
});
native_model!(FeishuSender {
    sender_id: FeishuSenderId = "sender_id",
    sender_type: String = "sender_type",
    tenant_key: String = "tenant_key",
});
native_model!(FeishuSenderId {
    open_id: String = "open_id",
    user_id: String = "user_id",
    union_id: String = "union_id",
});
native_model!(FeishuMessage {
    message_id: String = "message_id",
    message_type: String = "message_type",
    content: String = "content",
    chat_id: String = "chat_id",
    chat_type: String = "chat_type",
    create_time: String = "create_time",
    parent_id: String = "parent_id",
    root_id: String = "root_id",
});

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
        if !signed || object(request.body)?.get("encrypt").is_none() {
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
    let event_type = first(&[
        text(&payload, &["header", "event_type"]),
        text(&payload, &["type"]),
    ]);
    let id = first(&[
        text(&payload, &["header", "event_id"]),
        text(&payload, &["uuid"]),
    ]);
    Ok(PlatformEvent::from_raw(
        config.provider,
        metadata(event_type, optional(schema), "feishu:event", id),
        EventContent::Json(payload),
        request,
    )
    .into())
}

fn decode(body: &[u8], encrypt_key: Option<&str>) -> Result<Value, WebhookError> {
    let outer = json_payload(body)?;
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
        json_payload(&plaintext)
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

impl FeishuWebhook {
    pub fn to_message_content(
        &self,
        metadata: &EventMetadata,
        raw: &EventContent,
    ) -> MessageContent {
        let p = raw.as_json().unwrap_or(&Value::Null);
        let raw_content = text(p, &["event", "message", "content"]);
        let message_type = text(p, &["event", "message", "message_type"]);
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
