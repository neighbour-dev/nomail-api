#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidInput(String),
    NotFound,
    Conflict,
    Internal(String),
}
impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(s) | Self::Internal(s) => f.write_str(s),
            Self::NotFound => f.write_str("webhook config not found"),
            Self::Conflict => f.write_str("resource already exists"),
        }
    }
}
impl std::error::Error for ConfigError {}
pub type ConfigResult<T> = Result<T, ConfigError>;

impl crate::error::DomainError for ConfigError {
    fn kind(&self) -> crate::error::ErrorKind {
        use crate::error::ErrorKind;
        match self {
            Self::InvalidInput(_) => ErrorKind::InvalidInput,
            Self::NotFound => ErrorKind::NotFound,
            Self::Conflict => ErrorKind::Conflict,
            Self::Internal(_) => ErrorKind::Internal,
        }
    }
    fn domain(&self) -> &'static str {
        "webhook_config"
    }
    fn public_message(&self) -> &str {
        match self {
            Self::InvalidInput(s) => s,
            Self::NotFound => "webhook config not found",
            Self::Conflict => "resource already exists",
            Self::Internal(_) => crate::error::INTERNAL_MESSAGE,
        }
    }
}
