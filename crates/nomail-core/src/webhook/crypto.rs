//! Only reusable cryptographic primitives; header names and protocols live with platforms.
use super::{WebhookConfig, WebhookError, WebhookRequest};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

pub(super) fn equal(left: &[u8], right: &[u8]) -> Result<(), WebhookError> {
    if bool::from(left.ct_eq(right)) {
        Ok(())
    } else {
        Err(WebhookError::Unauthorized)
    }
}

pub(super) fn hmac(key: &[u8], parts: &[&[u8]], signature: &[u8]) -> Result<(), WebhookError> {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(key).map_err(|_| WebhookError::Unauthorized)?;
    for part in parts {
        mac.update(part);
    }
    mac.verify_slice(signature)
        .map_err(|_| WebhookError::Unauthorized)
}

pub(super) fn hex_signature(value: &str) -> Result<Vec<u8>, WebhookError> {
    hex::decode(value).map_err(|_| WebhookError::Unauthorized)
}

pub(super) fn timestamp(config: &WebhookConfig<'_>, value: &str) -> Result<(), WebhookError> {
    let value = value
        .parse::<i64>()
        .map_err(|_| WebhookError::Unauthorized)?;
    if value < 0 || config.now.abs_diff(value) > config.timestamp_tolerance {
        return Err(WebhookError::Unauthorized);
    }
    Ok(())
}

pub(super) fn hmac_header(
    secret: &str,
    name: &str,
    request: &WebhookRequest<'_>,
) -> Result<(), WebhookError> {
    let value = request
        .header(name)
        .strip_prefix("sha256=")
        .ok_or(WebhookError::Unauthorized)?;
    hmac(secret.as_bytes(), &[request.body], &hex_signature(value)?)
}
