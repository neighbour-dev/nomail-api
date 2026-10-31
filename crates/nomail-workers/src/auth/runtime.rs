use chrono::{DateTime, Utc};
use nomail_core::auth::{AuthError, AuthResult, AuthRuntime, password};

pub struct Runtime;
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(crate::time::now_ms()).expect("runtime timestamp in range")
    }
    fn random_bytes(&self, output: &mut [u8]) -> AuthResult<()> {
        getrandom::getrandom(output)
            .map_err(|e| AuthError::Internal(format!("randomness unavailable: {e}")))
    }
    async fn hash_password(&self, value: &str, salt: [u8; 16]) -> AuthResult<String> {
        password::hash(value, salt)
    }
    async fn verify_password(&self, value: &str, hash: &str) -> AuthResult<bool> {
        password::verify(value, hash)
    }
}
