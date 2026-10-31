use serde::{Deserialize, Serialize};

/// Application pagination result, independent of HTTP envelopes and routes.
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
impl<T> Page<T> {
    pub fn map<U>(self, f: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            page: self.page,
            page_size: self.page_size,
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

pub fn pagination(page: Option<&str>, size: Option<&str>) -> (u64, u64) {
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

pub fn summarize(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect()
}
