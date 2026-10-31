use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    InvalidInput(String),
    Unauthorized,
    NotFound(&'static str),
    Conflict(&'static str),
    Internal(String),
}
pub type AuthResult<T> = Result<T, AuthError>;
impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) | Self::Internal(message) => f.write_str(message),
            Self::Unauthorized => f.write_str("unauthorized"),
            Self::NotFound(message) | Self::Conflict(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for AuthError {}

impl crate::error::DomainError for AuthError {
    fn kind(&self) -> crate::error::ErrorKind {
        use crate::error::ErrorKind;
        match self {
            Self::InvalidInput(_) => ErrorKind::InvalidInput,
            Self::Unauthorized => ErrorKind::Unauthorized,
            Self::NotFound(_) => ErrorKind::NotFound,
            Self::Conflict(_) => ErrorKind::Conflict,
            Self::Internal(_) => ErrorKind::Internal,
        }
    }
    fn domain(&self) -> &'static str {
        "auth"
    }
    fn public_message(&self) -> &str {
        match self {
            Self::InvalidInput(s) => s,
            Self::NotFound(s) | Self::Conflict(s) => s,
            Self::Unauthorized => crate::error::UNAUTHORIZED_MESSAGE,
            Self::Internal(_) => crate::error::INTERNAL_MESSAGE,
        }
    }
}
