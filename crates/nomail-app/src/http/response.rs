use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "openapi", schemars(rename = "ApiResponse_{T}"))]
pub struct ApiResponse<T> {
    pub code: u16,
    pub msg: String,
    pub data: T,
}
impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            msg: "success".into(),
            data,
        }
    }
}
impl ApiResponse<()> {
    pub fn error(status: u16, message: impl Into<String>) -> Self {
        Self {
            code: status,
            msg: message.into(),
            data: (),
        }
    }
}

pub use crate::pagination::{AffectedResponse, CountResponse, Page, PaginationQuery};

/// NoMail HTTP acknowledgement. Platform controls keep their protocol response.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum ReceiveResponse {
    Message {
        #[serde(rename = "messageId")]
        message_id: uuid::Uuid,
        duplicate: bool,
    },
    Ignored {
        ignored: bool,
    },
}
#[derive(Serialize, Deserialize, Clone, Copy)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Error,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub db: HealthStatus,
}
impl HealthResponse {
    pub fn database(healthy: bool) -> Self {
        let status = if healthy {
            HealthStatus::Ok
        } else {
            HealthStatus::Error
        };
        Self { status, db: status }
    }
}
