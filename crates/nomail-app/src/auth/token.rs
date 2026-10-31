use super::{ACCESS_TTL, AuthConfig, AuthError, AuthResult, Claims};
use chrono::{DateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};

pub fn issue(
    config: &AuthConfig,
    user_id: i64,
    session_id: i64,
    now: DateTime<Utc>,
) -> AuthResult<(String, DateTime<Utc>)> {
    validate_config(config)?;
    let expires = now.timestamp() + ACCESS_TTL;
    let claims = Claims {
        user_id,
        session_id,
        sub: user_id.to_string(),
        iss: config.jwt_issuer.clone(),
        iat: now.timestamp(),
        exp: expires,
    };
    let value = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AuthError::Internal(format!("token signing failed: {e}")))?;
    Ok((
        value,
        DateTime::from_timestamp(expires, 0)
            .ok_or_else(|| AuthError::Internal("invalid token expiry".into()))?,
    ))
}

pub fn decode(config: &AuthConfig, token: &str, now: DateTime<Utc>) -> AuthResult<Claims> {
    validate_config(config)?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[&config.jwt_issuer]);
    validation.leeway = 0;
    // Keep signature/algorithm/issuer/required-claim checks in jsonwebtoken.
    // Expiry uses the caller's trusted clock so the use case never accesses a clock and
    // native/WASM/tests agree on the exact exp <= now boundary.
    validation.validate_exp = false;
    let claims = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| AuthError::Unauthorized)?
    .claims;
    if claims.user_id <= 0 || claims.session_id <= 0 || claims.exp <= now.timestamp() {
        return Err(AuthError::Unauthorized);
    }
    Ok(claims)
}

pub(super) fn validate_config(config: &AuthConfig) -> AuthResult<()> {
    if config.jwt_secret.is_empty() {
        Err(AuthError::Internal("JWT_SECRET is required".into()))
    } else {
        Ok(())
    }
}
