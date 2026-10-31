//! Final Workers response adaptation; shared HTTP errors already carry policy.
use nomail_app::{
    error::Diagnostic,
    http::{ApiFailure, ApiResponse, HttpError},
};
use worker::Response;

pub type Result<T> = std::result::Result<T, ApiError>;

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
impl From<worker::Error> for ApiError {
    fn from(_error: worker::Error) -> Self {
        // Platform transport failures are not database conflicts. D1 constraint
        // classification belongs exclusively to the storage adapters.
        ApiFailure::internal().into()
    }
}
impl ApiError {
    pub fn response(&self) -> worker::Result<Response> {
        let failure = &self.0.failure;
        if failure.status >= 500 {
            let diagnostic = self.0.diagnostic.unwrap_or(Diagnostic {
                domain: "http",
                message: "internal operation failed",
            });
            worker::console_error!(
                "request failed [{}]: {}",
                diagnostic.domain,
                diagnostic.message
            );
        }
        Response::from_json(&ApiResponse::error(failure.status, &failure.message))
            .map(|r| r.with_status(failure.status))
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
