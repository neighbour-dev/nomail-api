use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookError {
    InvalidConfiguration(&'static str),
    Unauthorized,
    InvalidPayload(&'static str),
    Unsupported(&'static str),
}
impl fmt::Display for WebhookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfiguration(message)
            | Self::InvalidPayload(message)
            | Self::Unsupported(message) => message,
            Self::Unauthorized => "unauthorized",
        })
    }
}
impl std::error::Error for WebhookError {}

impl crate::error::DomainError for WebhookError {
    fn kind(&self) -> crate::error::ErrorKind {
        use crate::error::ErrorKind;
        match self {
            Self::InvalidConfiguration(_) => ErrorKind::Internal,
            Self::Unauthorized => ErrorKind::Unauthorized,
            Self::InvalidPayload(_) | Self::Unsupported(_) => ErrorKind::InvalidInput,
        }
    }
    fn domain(&self) -> &'static str {
        "webhook"
    }
    fn public_message(&self) -> &str {
        match self {
            Self::InvalidConfiguration(_) => crate::error::INTERNAL_MESSAGE,
            Self::Unauthorized => crate::error::UNAUTHORIZED_MESSAGE,
            Self::InvalidPayload(s) | Self::Unsupported(s) => s,
        }
    }
}
