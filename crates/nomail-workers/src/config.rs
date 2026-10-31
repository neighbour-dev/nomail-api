use std::fmt;
use worker::Env;

/// Deployment configuration failures, independent of HTTP response policy.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigLoadError {
    MissingSecret(&'static str),
    EmptyJwtSecret,
    InvalidEncryptionKey,
}
impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSecret(name) => write!(f, "missing {name} binding"),
            Self::EmptyJwtSecret => f.write_str("JWT_SECRET is required"),
            Self::InvalidEncryptionKey => f.write_str("invalid encryption key configuration"),
        }
    }
}
impl std::error::Error for ConfigLoadError {}

pub struct Config {
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub encryption_key: [u8; 32],
}
impl Config {
    pub fn load(env: &Env) -> Result<Self, ConfigLoadError> {
        Self::from_secrets(
            |name| {
                env.secret(name)
                    .map(|v| v.to_string())
                    .map_err(|_| ConfigLoadError::MissingSecret(name))
            },
            env.var("JWT_ISSUER")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "nomail-api".into()),
        )
    }
    fn from_secrets(
        secret: impl Fn(&'static str) -> Result<String, ConfigLoadError>,
        jwt_issuer: String,
    ) -> Result<Self, ConfigLoadError> {
        let jwt_secret = secret("JWT_SECRET")?;
        if jwt_secret.is_empty() {
            return Err(ConfigLoadError::EmptyJwtSecret);
        }
        let encryption_key = nomail_app::security::decode_key(&secret("SECRET_ENCRYPTION_KEY")?)
            .map_err(|_| ConfigLoadError::InvalidEncryptionKey)?;
        Ok(Self {
            jwt_secret,
            jwt_issuer,
            encryption_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(values: &[(&str, &str)]) -> Result<Config, ConfigLoadError> {
        Config::from_secrets(
            |name| {
                values
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.to_string())
                    .ok_or(ConfigLoadError::MissingSecret(name))
            },
            "nomail-api".into(),
        )
    }
    #[test]
    fn configuration_failures_are_typed_and_do_not_include_secret_values() {
        assert_eq!(
            load(&[]).err(),
            Some(ConfigLoadError::MissingSecret("JWT_SECRET"))
        );
        assert_eq!(
            load(&[("JWT_SECRET", "")]).err(),
            Some(ConfigLoadError::EmptyJwtSecret)
        );
        assert_eq!(
            load(&[("JWT_SECRET", "private-jwt")]).err(),
            Some(ConfigLoadError::MissingSecret("SECRET_ENCRYPTION_KEY"))
        );
        for key in ["private-invalid-key", "", "YQ=="] {
            let error = load(&[
                ("JWT_SECRET", "private-jwt"),
                ("SECRET_ENCRYPTION_KEY", key),
            ])
            .err()
            .unwrap();
            assert_eq!(error, ConfigLoadError::InvalidEncryptionKey);
            assert!(!format!("{error} {error:?}").contains("private"));
        }
        let key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
        let config = load(&[
            ("JWT_SECRET", "private-jwt"),
            ("SECRET_ENCRYPTION_KEY", key),
        ])
        .unwrap();
        assert_eq!(config.encryption_key, [7; 32]);
        assert_eq!(config.jwt_secret, "private-jwt");
        assert_eq!(config.jwt_issuer, "nomail-api");
    }
}
