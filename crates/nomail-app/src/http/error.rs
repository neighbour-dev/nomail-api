//! The single HTTP mapping boundary for domain errors. No logging here.
use super::ApiResponse;
use crate::error::Diagnostic;
use crate::error::{
    AuthError, ConfigError, DomainError, ErrorKind, INTERNAL_MESSAGE, MessageError,
    UNAUTHORIZED_MESSAGE, WebhookError,
};
use crate::ingestion::IngestionError;

pub type HttpResult<T> = Result<T, HttpError>;

/// Preserve safe domain diagnostics until the final platform logging boundary.
#[derive(Clone, Debug)]
pub struct HttpError {
    pub failure: ApiFailure,
    pub diagnostic: Option<Diagnostic>,
}
impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure.fmt(f)
    }
}
impl std::error::Error for HttpError {}
impl From<ApiFailure> for HttpError {
    fn from(failure: ApiFailure) -> Self {
        Self {
            failure,
            diagnostic: None,
        }
    }
}
macro_rules! execution_errors {
    ($($error:ty),* $(,)?) => {$(
        impl From<$error> for HttpError {
            fn from(error: $error) -> Self {
                Self {
                    failure: ApiFailure::from_domain(&error),
                    diagnostic: error.diagnostic(),
                }
            }
        }
    )*};
}
execution_errors!(
    AuthError,
    MessageError,
    ConfigError,
    WebhookError,
    IngestionError
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiFailure {
    pub status: u16,
    pub message: String,
}
impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiFailure {}
impl ApiFailure {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }
    pub fn unauthorized() -> Self {
        Self::new(401, UNAUTHORIZED_MESSAGE)
    }
    pub fn not_found() -> Self {
        Self::new(404, "not found")
    }
    pub fn internal() -> Self {
        Self::new(500, INTERNAL_MESSAGE)
    }
    pub fn response(self) -> ApiResponse<()> {
        ApiResponse::error(self.status, self.message)
    }
    pub fn from_domain(error: &(impl DomainError + ?Sized)) -> Self {
        let status = match error.kind() {
            ErrorKind::InvalidInput => 400,
            ErrorKind::Unauthorized => 401,
            ErrorKind::NotFound => 404,
            ErrorKind::Conflict => 409,
            ErrorKind::Internal => return Self::internal(),
        };
        Self::new(status, error.public_message())
    }
}

// Concrete conversions preserve the existing API without a blanket From<E>
// implementation that could overlap other transport conversions.
impl From<AuthError> for ApiFailure {
    fn from(error: AuthError) -> Self {
        Self::from_domain(&error)
    }
}
impl From<MessageError> for ApiFailure {
    fn from(error: MessageError) -> Self {
        Self::from_domain(&error)
    }
}
impl From<ConfigError> for ApiFailure {
    fn from(error: ConfigError) -> Self {
        Self::from_domain(&error)
    }
}
impl From<WebhookError> for ApiFailure {
    fn from(error: WebhookError) -> Self {
        Self::from_domain(&error)
    }
}
impl From<IngestionError> for ApiFailure {
    fn from(error: IngestionError) -> Self {
        Self::from_domain(&error)
    }
}
