use crate::{crypto, error::ApiError};
use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_path: String,
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub encryption_key: [u8; 32],
    pub port: u16,
    pub initial_username: String,
    pub initial_password: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ApiError> {
        let get = |key: &str, default: &str| env::var(key).unwrap_or_else(|_| default.into());
        let jwt_secret = get("JWT_SECRET", "");
        if jwt_secret.is_empty() {
            return Err(ApiError::bad("JWT_SECRET is required"));
        }
        Ok(Self {
            database_path: get("DATABASE_PATH", "./data/nomail.db"),
            jwt_secret,
            jwt_issuer: get("JWT_ISSUER", "nomail-api"),
            encryption_key: crypto::decode_key(&get("SECRET_ENCRYPTION_KEY", ""))?,
            port: get("PORT", "3000").parse().map_err(ApiError::internal)?,
            initial_username: get("INITIAL_USERNAME", ""),
            initial_password: get("INITIAL_PASSWORD", ""),
        })
    }
}
