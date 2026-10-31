//! One bcrypt policy, executed on a blocking pool or in Workers by adapters.
use super::{AuthError, AuthResult};
pub const BCRYPT_COST: u32 = 10;

pub fn hash(password: &str, salt: [u8; 16]) -> AuthResult<String> {
    crate::security::validate_password(password).map_err(|s| AuthError::InvalidInput(s.into()))?;
    bcrypt::hash_with_salt(password, BCRYPT_COST, salt)
        .map(|parts| parts.format_for_version(bcrypt::Version::TwoB))
        .map_err(|_| AuthError::Internal("password hashing failed".into()))
}
pub fn verify(password: &str, hash: &str) -> AuthResult<bool> {
    // bcrypt's error Display may contain the supplied hash; never log it.
    bcrypt::verify(password, hash)
        .map_err(|_| AuthError::Internal("invalid stored password hash".into()))
}
