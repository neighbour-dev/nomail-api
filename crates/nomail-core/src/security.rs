//! Portable credential encoding. Randomness is supplied by the runtime.
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};

pub fn decode_key(value: &str) -> Result<[u8; 32], &'static str> {
    STANDARD
        .decode(value)
        .map_err(|_| "invalid base64 encryption key")?
        .try_into()
        .map_err(|_| "encryption key must be 32 bytes")
}

pub fn hash_token(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn encrypt(value: &str, key: &[u8; 32], nonce: &[u8; 12]) -> Result<String, &'static str> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "invalid encryption key")?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(nonce), value.as_bytes())
        .map_err(|_| "encryption failed")?;
    Ok(STANDARD.encode([nonce.as_slice(), &encrypted].concat()))
}

pub fn decrypt(value: &str, key: &[u8; 32]) -> Result<String, &'static str> {
    let bytes = STANDARD.decode(value).map_err(|_| "invalid ciphertext")?;
    if bytes.len() < 28 {
        return Err("invalid ciphertext");
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "invalid encryption key")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&bytes[..12]), &bytes[12..])
        .map_err(|_| "decryption failed")?;
    String::from_utf8(plaintext).map_err(|_| "invalid plaintext")
}

pub fn validate_password(value: &str) -> Result<(), &'static str> {
    if value.chars().count() < 8
        || !value.bytes().any(|c| c.is_ascii_alphabetic())
        || !value.bytes().any(|c| c.is_ascii_digit())
    {
        return Err("password must be at least 8 characters and contain letters and numbers");
    }
    if value.len() > 72 {
        return Err("password must not exceed 72 bytes");
    }
    Ok(())
}
