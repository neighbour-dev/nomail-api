use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use std::fmt;

pub type ApiResult = Result<HttpResponse, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn bad(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "unauthorized".into(),
        }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }
    pub fn internal(error: impl fmt::Display) -> Self {
        tracing::error!(error = %error, "request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".into(),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiError {}
impl From<nomail_core::webhook::WebhookError> for ApiError {
    fn from(error: nomail_core::webhook::WebhookError) -> Self {
        use nomail_core::webhook::WebhookError;
        match error {
            WebhookError::Unauthorized => Self::unauthorized(),
            WebhookError::InvalidConfiguration(_) => Self::internal(error),
            WebhookError::InvalidPayload(_) | WebhookError::Unsupported(_) => {
                Self::bad(error.to_string())
            }
        }
    }
}
impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.status
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status).json(serde_json::json!({
            "code": self.status.as_u16(), "msg": self.message, "data": null
        }))
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
pub fn success(data: impl Serialize) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"code": 0, "msg": "success", "data": data}))
}
pub fn check_length(value: &str, name: &str, max: usize, required: bool) -> Result<(), ApiError> {
    if (required && value.is_empty()) || value.chars().count() > max {
        return Err(ApiError::bad(format!("invalid {name}")));
    }
    Ok(())
}
