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

pub(super) fn default_page() -> u64 {
    1
}

pub(super) fn default_page_size() -> u64 {
    20
}
