use chrono::{DateTime, Utc};
use nomail_app::auth::{AuthError, AuthResult, AuthRuntime, password};
use rand::{RngCore, rngs::OsRng};

pub struct Runtime;
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn random_bytes(&self, output: &mut [u8]) -> AuthResult<()> {
        OsRng
            .try_fill_bytes(output)
            .map_err(|e| AuthError::Internal(format!("randomness unavailable: {e}")))
    }
    async fn hash_password(&self, value: &str, salt: [u8; 16]) -> AuthResult<String> {
        let value = value.to_owned();
        tokio::task::spawn_blocking(move || password::hash(&value, salt))
            .await
            .map_err(|e| AuthError::Internal(e.to_string()))?
    }
    async fn verify_password(&self, value: &str, hash: &str) -> AuthResult<bool> {
        let (value, hash) = (value.to_owned(), hash.to_owned());
        tokio::task::spawn_blocking(move || password::verify(&value, &hash))
            .await
            .map_err(|e| AuthError::Internal(e.to_string()))?
    }
}
