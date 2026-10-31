//! Classify protocol errors for the community product, not inside the protocol library.
use super::{DomainError, ErrorKind, INTERNAL_MESSAGE, UNAUTHORIZED_MESSAGE};
use nomail_core::webhook::WebhookError;

impl DomainError for WebhookError {
    fn kind(&self) -> ErrorKind {
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
            Self::InvalidConfiguration(_) => INTERNAL_MESSAGE,
            Self::Unauthorized => UNAUTHORIZED_MESSAGE,
            Self::InvalidPayload(s) | Self::Unsupported(s) => s,
        }
    }
}
