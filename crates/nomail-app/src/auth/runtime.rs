use super::AuthResult;
use chrono::{DateTime, Utc};

/// Runtime capabilities only. Algorithm/format/cost live in auth::password.
#[allow(async_fn_in_trait)]
pub trait AuthRuntime {
    fn now(&self) -> DateTime<Utc>;
    fn random_bytes(&self, output: &mut [u8]) -> AuthResult<()>;
    async fn hash_password(&self, password: &str, salt: [u8; 16]) -> AuthResult<String>;
    async fn verify_password(&self, password: &str, hash: &str) -> AuthResult<bool>;
}
