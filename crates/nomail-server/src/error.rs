use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use nomail_core::api::{ApiFailure, ApiResponse};
use nomail_core::error::{Diagnostic, DomainError};
use std::fmt;

pub type ApiResult = Result<HttpResponse, ApiError>;

impl From<nomail_core::webhook::config::ConfigError> for ApiError {
    fn from(error: nomail_core::webhook::config::ConfigError) -> Self {
        Self::from_domain(error)
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    diagnostic: Option<Diagnostic>,
}

impl ApiError {
    fn from_domain(error: impl DomainError) -> Self {
        let diagnostic = error.diagnostic();
        let mut result = Self::from(ApiFailure::from_domain(&error));
        result.diagnostic = diagnostic;
        result
    }
    pub fn bad(message: impl Into<String>) -> Self {
        ApiFailure::bad(message).into()
    }
    pub fn unauthorized() -> Self {
        ApiFailure::unauthorized().into()
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        ApiFailure::new(404, message).into()
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        ApiFailure::new(409, message).into()
    }
    pub fn internal(_error: impl fmt::Display) -> Self {
        // Do not automatically log arbitrary storage/runtime Display text.
        ApiFailure::internal().into()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiError {}
impl From<ApiFailure> for ApiError {
    fn from(error: ApiFailure) -> Self {
        Self {
            status: StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message: error.message,
            diagnostic: None,
        }
    }
}
impl From<nomail_core::auth::AuthError> for ApiError {
    fn from(error: nomail_core::auth::AuthError) -> Self {
        Self::from_domain(error)
    }
}
impl From<nomail_core::message::MessageError> for ApiError {
    fn from(error: nomail_core::message::MessageError) -> Self {
        Self::from_domain(error)
    }
}
impl From<nomail_core::webhook::WebhookError> for ApiError {
    fn from(error: nomail_core::webhook::WebhookError) -> Self {
        Self::from_domain(error)
    }
}
impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.status
    }
    fn error_response(&self) -> HttpResponse {
        // Log once at the response boundary, never during intermediate
        // storage -> domain -> transport conversions.
        if self.status.is_server_error() {
            let diagnostic = self.diagnostic.unwrap_or(Diagnostic {
                domain: "http",
                message: "internal operation failed",
            });
            tracing::error!(
                domain = diagnostic.domain,
                error = diagnostic.message,
                "request failed"
            );
        }
        HttpResponse::build(self.status)
            .json(ApiResponse::error(self.status.as_u16(), &self.message))
    }
}
impl From<sea_orm::DbErr> for ApiError {
    fn from(error: sea_orm::DbErr) -> Self {
        if matches!(
            error.sql_err(),
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
        ) {
            Self::conflict("resource already exists")
        } else {
            Self::internal(error)
        }
    }
}
pub fn success<E: nomail_core::api::Endpoint>(data: E::Response) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(data))
}
pub fn check_length(value: &str, name: &str, max: usize, required: bool) -> Result<(), ApiError> {
    nomail_core::auth::check_length(value, name, max, required).map_err(ApiError::bad)
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
