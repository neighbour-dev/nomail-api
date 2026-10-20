use crate::error::ApiError;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

pub fn decode_key(value: &str) -> Result<[u8; 32], ApiError> {
    STANDARD
        .decode(value)
        .map_err(|_| ApiError::bad("invalid base64 encryption key"))?
        .try_into()
        .map_err(|_| ApiError::bad("encryption key must be 32 bytes"))
}
pub fn random_token(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    OsRng.fill_bytes(&mut value);
    hex::encode(value)
}
pub fn hash_token(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

// Wire format shared with existing installations: Base64(nonce[12] || ciphertext || tag[16]).
pub fn encrypt(value: &str, key: &[u8; 32]) -> Result<String, ApiError> {
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    let cipher = Aes256Gcm::new_from_slice(key).map_err(ApiError::internal)?;
    let encrypted = cipher
        .encrypt(Nonce::from_slice(&nonce), value.as_bytes())
        .map_err(ApiError::internal)?;
    Ok(STANDARD.encode([nonce.as_slice(), &encrypted].concat()))
}
pub fn decrypt(value: &str, key: &[u8; 32]) -> Result<String, ApiError> {
    let bytes = STANDARD.decode(value).map_err(ApiError::internal)?;
    if bytes.len() < 28 {
        return Err(ApiError::internal("invalid ciphertext"));
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(ApiError::internal)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&bytes[..12]), &bytes[12..])
        .map_err(ApiError::internal)?;
    String::from_utf8(plaintext).map_err(ApiError::internal)
}
pub fn validate_password(value: &str) -> Result<(), ApiError> {
    if value.chars().count() < 8
        || !value.bytes().any(|c| c.is_ascii_alphabetic())
        || !value.bytes().any(|c| c.is_ascii_digit())
    {
        return Err(ApiError::bad(
            "password must be at least 8 characters and contain letters and numbers",
        ));
    }
    // bcrypt cannot safely represent passwords longer than 72 bytes.
    if value.len() > 72 {
        return Err(ApiError::bad("password must not exceed 72 bytes"));
    }
    Ok(())
}
pub async fn hash_password(value: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || bcrypt::hash(value, 10))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)
}
pub async fn verify_password(value: String, hash: String) -> Result<(), ApiError> {
    let valid = tokio::task::spawn_blocking(move || bcrypt::verify(value, &hash))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    if !valid {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encryption_roundtrip_and_authentication() {
        let key = [42; 32];
        let encrypted = encrypt("secret 中文", &key).unwrap();
        assert_eq!(decrypt(&encrypted, &key).unwrap(), "secret 中文");
        assert!(decrypt(&encrypted, &[43; 32]).is_err());
        assert!(decrypt("YQ==", &key).is_err());
    }
    #[test]
    fn password_rules_and_hashing() {
        for invalid in ["short1", "password", "12345678", "中文密码没有数字"] {
            assert!(validate_password(invalid).is_err());
        }
        assert!(validate_password("password123").is_ok());
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(decode_key(&STANDARD.encode([1; 32])).is_ok());
        assert!(decode_key(&STANDARD.encode([1; 31])).is_err());
    }
}
