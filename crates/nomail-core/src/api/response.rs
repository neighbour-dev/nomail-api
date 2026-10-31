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

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", schemars(rename = "Page_{T}"))]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}
#[derive(Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery {
    #[cfg_attr(feature = "openapi", schemars(with = "Option<u64>"))]
    pub page: Option<String>,
    #[cfg_attr(feature = "openapi", schemars(with = "Option<u64>"))]
    pub page_size: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct CountResponse {
    pub count: u64,
}
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(schemars::JsonSchema))]
pub struct AffectedResponse {
    pub affected: u64,
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
