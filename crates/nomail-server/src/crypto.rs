use crate::error::ApiError;
use rand::{RngCore, rngs::OsRng};

pub fn decode_key(value: &str) -> Result<[u8; 32], ApiError> {
    nomail_core::security::decode_key(value).map_err(ApiError::bad)
}
pub fn random_token(bytes: usize) -> String {
    let mut value = vec![0; bytes];
    OsRng.fill_bytes(&mut value);
    hex::encode(value)
}
pub fn hash_token(value: &str) -> String {
    nomail_core::security::hash_token(value)
}

// Wire format shared with existing installations: Base64(nonce[12] || ciphertext || tag[16]).
pub fn encrypt(value: &str, key: &[u8; 32]) -> Result<String, ApiError> {
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    nomail_core::security::encrypt(value, key, &nonce).map_err(ApiError::internal)
}
pub fn decrypt(value: &str, key: &[u8; 32]) -> Result<String, ApiError> {
    nomail_core::security::decrypt(value, key).map_err(ApiError::internal)
}
pub fn validate_password(value: &str) -> Result<(), ApiError> {
    nomail_core::security::validate_password(value).map_err(ApiError::bad)
}
pub async fn hash_password(value: String) -> Result<String, ApiError> {
    use nomail_core::auth::AuthRuntime;
    let runtime = crate::auth::runtime::Runtime;
    let mut salt = [0; 16];
    runtime.random_bytes(&mut salt)?;
    Ok(runtime.hash_password(&value, salt).await?)
}
pub async fn verify_password(value: String, hash: String) -> Result<(), ApiError> {
    use nomail_core::auth::AuthRuntime;
    let valid = crate::auth::runtime::Runtime
        .verify_password(&value, &hash)
        .await?;
    if !valid {
        return Err(ApiError::unauthorized());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
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
