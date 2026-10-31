use nomail_app::error::{Diagnostic, DomainError};
use nomail_app::http::{ApiFailure, ApiResponse};
use worker::Response;

pub type Result<T> = std::result::Result<T, ApiError>;
impl From<nomail_app::ingestion::IngestionError> for ApiError {
    fn from(error: nomail_app::ingestion::IngestionError) -> Self {
        Self::from_domain(error)
    }
}

impl From<nomail_app::webhook::config::ConfigError> for ApiError {
    fn from(error: nomail_app::webhook::config::ConfigError) -> Self {
        Self::from_domain(error)
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
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
    pub fn response(&self) -> worker::Result<Response> {
        if self.status >= 500 {
            let diagnostic = self.diagnostic.unwrap_or(Diagnostic {
                domain: "http",
                message: "internal operation failed",
            });
            worker::console_error!(
                "request failed [{}]: {}",
                diagnostic.domain,
                diagnostic.message
            );
        }
        Response::from_json(&ApiResponse::error(self.status, &self.message))
            .map(|r| r.with_status(self.status))
    }
}
impl From<ApiFailure> for ApiError {
    fn from(error: ApiFailure) -> Self {
        Self {
            status: error.status,
            message: error.message,
            diagnostic: None,
        }
    }
}
impl From<worker::Error> for ApiError {
    fn from(error: worker::Error) -> Self {
        if error.to_string().contains("UNIQUE constraint failed") {
            return ApiFailure::new(409, "resource already exists").into();
        }
        ApiFailure::internal().into()
    }
}
impl From<nomail_app::auth::AuthError> for ApiError {
    fn from(error: nomail_app::auth::AuthError) -> Self {
        Self::from_domain(error)
    }
}
impl From<nomail_app::message::MessageError> for ApiError {
    fn from(error: nomail_app::message::MessageError) -> Self {
        Self::from_domain(error)
    }
}
impl From<nomail_app::webhook::WebhookError> for ApiError {
    fn from(error: nomail_app::webhook::WebhookError) -> Self {
        Self::from_domain(error)
    }
}
pub fn success<E: nomail_app::http::Endpoint>(data: E::Response) -> Result<Response> {
    Ok(Response::from_json(&ApiResponse::success(data))?)
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
