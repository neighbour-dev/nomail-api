use crate::{
    error::{DomainError, ErrorKind, INTERNAL_MESSAGE},
    message::MessageError,
    webhook::WebhookError,
};
use std::fmt;

pub type IngestionResult<T> = Result<T, IngestionError>;
#[derive(Debug)]
pub enum IngestionError {
    ConfigNotFound,
    Protocol(WebhookError),
    Message(MessageError),
    Internal(String),
}
impl fmt::Display for IngestionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigNotFound => f.write_str("webhook config not found"),
            Self::Protocol(error) => error.fmt(f),
            Self::Message(error) => error.fmt(f),
            Self::Internal(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for IngestionError {}
impl DomainError for IngestionError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::ConfigNotFound => ErrorKind::NotFound,
            Self::Protocol(error) => error.kind(),
            Self::Message(error) => error.kind(),
            Self::Internal(_) => ErrorKind::Internal,
        }
    }
    fn domain(&self) -> &'static str {
        "ingestion"
    }
    fn public_message(&self) -> &str {
        match self {
            Self::ConfigNotFound => "webhook config not found",
            Self::Protocol(error) => error.public_message(),
            Self::Message(error) => error.public_message(),
            Self::Internal(_) => INTERNAL_MESSAGE,
        }
    }
}
impl From<WebhookError> for IngestionError {
    fn from(error: WebhookError) -> Self {
        Self::Protocol(error)
    }
}
impl From<MessageError> for IngestionError {
    fn from(error: MessageError) -> Self {
        Self::Message(error)
    }
}
