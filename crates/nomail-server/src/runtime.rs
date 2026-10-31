//! Native execution capabilities shared by HTTP services and account maintenance.
use chrono::{DateTime, Utc};
use nomail_app::{
    auth::{AuthError, AuthResult, AuthRuntime, password},
    ingestion::{IngestionResult, IngestionRuntime},
    webhook::config::{ConfigError, ConfigResult, WebhookConfigRuntime},
};
use rand::{RngCore, rngs::OsRng};

#[derive(Clone)]
pub(crate) struct Runtime {
    pub encryption_key: [u8; 32],
}

fn random_bytes(output: &mut [u8]) -> Result<(), rand::Error> {
    OsRng.try_fill_bytes(output)
}
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
    fn random_bytes(&self, output: &mut [u8]) -> AuthResult<()> {
        random_bytes(output)
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

impl WebhookConfigRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
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
        Utc::now()
    }
    fn message_id(&self, _now: DateTime<Utc>) -> IngestionResult<uuid::Uuid> {
        Ok(uuid::Uuid::now_v7())
    }
    fn encryption_key(&self) -> &[u8; 32] {
        &self.encryption_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_runtime_supplies_all_use_cases_without_auth_error_forwarding() {
        let runtime = Runtime {
            encryption_key: [7; 32],
        };
        let mut bytes = [0; 16];
        AuthRuntime::random_bytes(&runtime, &mut bytes).unwrap();
        WebhookConfigRuntime::random_bytes(&runtime, &mut bytes).unwrap();
        assert_eq!(WebhookConfigRuntime::encryption_key(&runtime), &[7; 32]);
        assert_eq!(IngestionRuntime::encryption_key(&runtime), &[7; 32]);
        let id = IngestionRuntime::message_id(&runtime, Utc::now()).unwrap();
        assert_eq!(id.get_version_num(), 7);
    }
}
