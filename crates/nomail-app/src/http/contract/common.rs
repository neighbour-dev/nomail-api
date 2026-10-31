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

/// Wire pagination envelope, separate from the application page.
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
impl<T, U: From<T>> From<crate::pagination::Page<T>> for Page<U> {
    fn from(page: crate::pagination::Page<T>) -> Self {
        Self {
            items: page.items.into_iter().map(U::from).collect(),
            total: page.total,
            page: page.page,
            page_size: page.page_size,
        }
    }
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

pub(crate) fn parse_pagination(page: Option<&str>, size: Option<&str>) -> (u64, u64) {
    let page = page
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1) as u64;
    let size = size
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(20)
        .min(100) as u64;
    (page, size)
}
impl TryFrom<PaginationQuery> for crate::webhook::config::ConfigPage {
    type Error = crate::webhook::config::ConfigError;
    fn try_from(query: PaginationQuery) -> Result<Self, Self::Error> {
        let (page, size) = parse_pagination(query.page.as_deref(), query.page_size.as_deref());
        Self::new(page, size)
    }
}

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
