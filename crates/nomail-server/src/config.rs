use std::{env, fmt};

/// Startup configuration errors, never HTTP responses or secret values.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigLoadError {
    MissingJwtSecret,
    EncryptionKey(&'static str),
    InvalidPort,
}
impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingJwtSecret => f.write_str("JWT_SECRET is required"),
            Self::EncryptionKey(message) => write!(f, "invalid SECRET_ENCRYPTION_KEY: {message}"),
            Self::InvalidPort => {
                f.write_str("invalid PORT: expected an integer between 0 and 65535")
            }
        }
    }
}
impl std::error::Error for ConfigLoadError {}

#[derive(Clone)]
pub struct Config {
    pub database_path: String,
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub encryption_key: [u8; 32],
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigLoadError> {
        Self::from_lookup(|key, default| env::var(key).unwrap_or_else(|_| default.into()))
    }
    fn from_lookup(get: impl Fn(&str, &str) -> String) -> Result<Self, ConfigLoadError> {
        let jwt_secret = get("JWT_SECRET", "");
        if jwt_secret.is_empty() {
            return Err(ConfigLoadError::MissingJwtSecret);
        }
        Ok(Self {
            database_path: get("DATABASE_PATH", "./data/nomail.db"),
            jwt_secret,
            jwt_issuer: get("JWT_ISSUER", "nomail-api"),
            encryption_key: nomail_app::security::decode_key(&get("SECRET_ENCRYPTION_KEY", ""))
                .map_err(ConfigLoadError::EncryptionKey)?,
            port: get("PORT", "3000")
                .parse()
                .map_err(|_| ConfigLoadError::InvalidPort)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};

    fn load(values: &[(&str, &str)]) -> Result<Config, ConfigLoadError> {
        Config::from_lookup(|key, default| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map_or(default, |(_, value)| *value)
                .to_owned()
        })
    }
    #[test]
    fn startup_validation_preserves_defaults_and_reports_fields_without_values() {
        assert_eq!(load(&[]).err(), Some(ConfigLoadError::MissingJwtSecret));
        let key = STANDARD.encode([7; 32]);
        let config = load(&[
            ("JWT_SECRET", "private-jwt"),
            ("SECRET_ENCRYPTION_KEY", &key),
        ])
        .unwrap();
        assert_eq!(config.port, 3000);
        assert_eq!(config.database_path, "./data/nomail.db");
        assert_eq!(config.jwt_issuer, "nomail-api");
        assert_eq!(config.encryption_key, [7; 32]);
        for port in ["not-a-port", "-1", "65536"] {
            let error = load(&[
                ("JWT_SECRET", "private-jwt"),
                ("SECRET_ENCRYPTION_KEY", &key),
                ("PORT", port),
            ])
            .err()
            .unwrap();
            assert_eq!(error, ConfigLoadError::InvalidPort);
            assert!(error.to_string().contains("PORT"));
            assert!(!format!("{error:?}").contains("private-jwt"));
        }
        let error = load(&[
            ("JWT_SECRET", "private-jwt"),
            ("SECRET_ENCRYPTION_KEY", "private-invalid-key"),
        ])
        .err()
        .unwrap();
        assert!(matches!(error, ConfigLoadError::EncryptionKey(_)));
        assert!(error.to_string().contains("SECRET_ENCRYPTION_KEY"));
        assert!(!format!("{error} {error:?}").contains("private-invalid-key"));
    }
}
