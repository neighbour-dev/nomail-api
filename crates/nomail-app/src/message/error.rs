use std::fmt;
#[derive(Debug, PartialEq, Eq)]
pub enum MessageError {
    InvalidInput(String),
    NotFound(&'static str),
    Conflict(&'static str),
    Internal(String),
}
pub type MessageResult<T> = Result<T, MessageError>;
impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) | Self::Internal(s) => f.write_str(s),
            Self::NotFound(s) | Self::Conflict(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for MessageError {}

impl crate::error::DomainError for MessageError {
    fn kind(&self) -> crate::error::ErrorKind {
        use crate::error::ErrorKind;
        match self {
            Self::InvalidInput(_) => ErrorKind::InvalidInput,
            Self::NotFound(_) => ErrorKind::NotFound,
            Self::Conflict(_) => ErrorKind::Conflict,
            Self::Internal(_) => ErrorKind::Internal,
        }
    }
    fn domain(&self) -> &'static str {
        "message"
    }
    fn public_message(&self) -> &str {
        match self {
            Self::InvalidInput(s) => s,
            Self::NotFound(s) | Self::Conflict(s) => s,
            Self::Internal(_) => crate::error::INTERNAL_MESSAGE,
        }
    }
}
