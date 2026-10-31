//! Workers execution capabilities, without an authentication-specific wrapper.
use chrono::{DateTime, Utc};
use nomail_app::{
    auth::{AuthError, AuthResult, AuthRuntime, password},
    ingestion::{IngestionError, IngestionResult, IngestionRuntime},
    webhook::config::{ConfigError, ConfigResult, WebhookConfigRuntime},
};

#[derive(Clone)]
pub(crate) struct Runtime {
    pub encryption_key: [u8; 32],
}

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(crate::time::now_ms()).expect("runtime timestamp in range")
}
fn random_bytes(output: &mut [u8]) -> Result<(), getrandom::Error> {
    getrandom::getrandom(output)
}
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        now()
    }
    fn random_bytes(&self, output: &mut [u8]) -> AuthResult<()> {
        random_bytes(output)
            .map_err(|e| AuthError::Internal(format!("randomness unavailable: {e}")))
    }
    async fn hash_password(&self, value: &str, salt: [u8; 16]) -> AuthResult<String> {
        password::hash(value, salt)
    }
    async fn verify_password(&self, value: &str, hash: &str) -> AuthResult<bool> {
        password::verify(value, hash)
    }
}

impl WebhookConfigRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        now()
    }
    fn random_bytes(&self, output: &mut [u8]) -> ConfigResult<()> {
        random_bytes(output)
            .map_err(|e| ConfigError::Internal(format!("randomness unavailable: {e}")))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}
impl IngestionRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        now()
    }
    fn message_id(&self, now: DateTime<Utc>) -> IngestionResult<uuid::Uuid> {
        crate::crypto::message_id(now.timestamp_millis() as u64)
            .map_err(|_| IngestionError::Internal("message identity generation failed".into()))
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_runtime_shares_the_key_and_preserves_supplied_message_time() {
        let runtime = Runtime {
            encryption_key: [7; 32],
        };
        let mut bytes = [0; 16];
        AuthRuntime::random_bytes(&runtime, &mut bytes).unwrap();
        WebhookConfigRuntime::random_bytes(&runtime, &mut bytes).unwrap();
        assert_eq!(WebhookConfigRuntime::encryption_key(&runtime), &[7; 32]);
        assert_eq!(IngestionRuntime::encryption_key(&runtime), &[7; 32]);
        let timestamp = 1_800_000_000_123_u64;
        let now = DateTime::from_timestamp_millis(timestamp as i64).unwrap();
        let id = IngestionRuntime::message_id(&runtime, now).unwrap();
        assert_eq!(&id.as_bytes()[..6], &timestamp.to_be_bytes()[2..]);
        assert_eq!(id.get_version_num(), 7);
    }
}
