use crate::error::{DomainError, ErrorKind, INTERNAL_MESSAGE, UNAUTHORIZED_MESSAGE};
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum DeviceError {
    InvalidInput(String),
    Unauthorized,
    NotFound,
    Conflict,
    Internal,
}

pub type DeviceResult<T> = Result<T, DeviceError>;

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.public_message())
    }
}

impl std::error::Error for DeviceError {}

impl DomainError for DeviceError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::InvalidInput(_) => ErrorKind::InvalidInput,
            Self::Unauthorized => ErrorKind::Unauthorized,
            Self::NotFound => ErrorKind::NotFound,
            Self::Conflict => ErrorKind::Conflict,
            Self::Internal => ErrorKind::Internal,
        }
    }

    fn domain(&self) -> &'static str {
        "device"
    }

    fn public_message(&self) -> &str {
        match self {
            Self::InvalidInput(message) => message,
            Self::Unauthorized => UNAUTHORIZED_MESSAGE,
            Self::NotFound => "device not found",
            Self::Conflict => "resource already exists",
            Self::Internal => INTERNAL_MESSAGE,
        }
    }
}
