use crate::error::{ApiError, Result};
use worker::Env;

pub struct Config {
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub encryption_key: [u8; 32],
}
impl Config {
    pub fn load(env: &Env) -> Result<Self> {
        let jwt_secret = env.secret("JWT_SECRET")?.to_string();
        if jwt_secret.is_empty() {
            return Err(ApiError::bad("JWT_SECRET is required"));
        }
        let encryption_key =
            nomail_app::security::decode_key(&env.secret("SECRET_ENCRYPTION_KEY")?.to_string())
                .map_err(|_| {
                    ApiError::from(nomail_app::http::ApiFailure::new(
                        500,
                        "invalid encryption key configuration",
                    ))
                })?;
        Ok(Self {
            jwt_secret,
            jwt_issuer: env
                .var("JWT_ISSUER")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "nomail-api".into()),
            encryption_key,
        })
    }
}
