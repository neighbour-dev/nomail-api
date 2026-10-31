//! Actix adaptation only. Domain-to-HTTP policy lives in nomail-app::http.
use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use nomail_app::{
    error::Diagnostic,
    http::{ApiFailure, ApiResponse, HttpError},
};
use std::fmt;

pub type ApiResult = Result<HttpResponse, ApiError>;

#[derive(Debug)]
pub struct ApiError(HttpError);

impl From<HttpError> for ApiError {
    fn from(error: HttpError) -> Self {
        Self(error)
    }
}
impl From<ApiFailure> for ApiError {
    fn from(failure: ApiFailure) -> Self {
        Self(failure.into())
    }
}
impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for ApiError {}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.0.failure.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        // Log once at the response boundary, never during intermediate
        // storage -> domain -> transport conversions.
        if status.is_server_error() {
            let diagnostic = self.0.diagnostic.unwrap_or(Diagnostic {
                domain: "http",
                message: "internal operation failed",
            });
            tracing::error!(
                domain = diagnostic.domain,
                error = diagnostic.message,
                "request failed"
            );
        }
        HttpResponse::build(status)
            .json(ApiResponse::error(status.as_u16(), &self.0.failure.message))
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
