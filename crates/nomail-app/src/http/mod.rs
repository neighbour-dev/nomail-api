//! HTTP contract, independent of any server framework or transport runtime.
mod config;
mod dispatch;
mod error;
mod response;
mod routes;
mod services;
pub mod sse;
mod transport;
pub mod webhook;
pub use config::ConfigResponse;
pub use dispatch::execute;
pub use error::{ApiFailure, HttpError, HttpResult};
pub use response::*;
pub use routes::*;
pub use services::{Environment, EnvironmentServices, HealthCheck, Services};
pub use transport::{
    HttpOutcome, HttpReply, HttpRequest, HttpResponse, RealtimeEffect, RequestHead, Subscription,
};
#[cfg(feature = "openapi")]
pub mod openapi;

use serde::de::DeserializeOwned;

pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// JSON business success only. Route.response identifies stream/raw/control
/// responses which intentionally do not use this envelope.
pub type EndpointResponse<E> = ApiResponse<<E as Endpoint>::Response>;

pub trait QueryDto: DeserializeOwned {
    fn normalize(&mut self) -> Result<(), ApiFailure> {
        Ok(())
    }
}
impl QueryDto for () {}
impl QueryDto for PaginationQuery {}
impl QueryDto for crate::message::ListQuery {
    fn normalize(&mut self) -> Result<(), ApiFailure> {
        crate::message::ListQuery::normalize(self).map_err(ApiFailure::from)
    }
}

pub fn check_body_length(length: usize) -> Result<(), ApiFailure> {
    if length > MAX_BODY_BYTES {
        Err(ApiFailure::new(413, "request body too large"))
    } else {
        Ok(())
    }
}
pub fn bearer_token(header: Option<&str>) -> Result<&str, ApiFailure> {
    header
        .and_then(|s| s.strip_prefix("Bearer "))
        .filter(|s| !s.is_empty())
        .ok_or_else(ApiFailure::unauthorized)
}
pub fn parse_json<E: Endpoint>(
    bytes: &[u8],
    content_type: Option<&str>,
) -> Result<E::Request, ApiFailure> {
    if E::OPERATION.route().body != BodyKind::Json {
        return Err(ApiFailure::internal());
    }
    check_body_length(bytes.len())?;
    let media = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if media != "application/json"
        && !(media.starts_with("application/") && media.ends_with("+json"))
    {
        return Err(ApiFailure::bad("content type must be application/json"));
    }
    // Deserialize the DTO directly so duplicate known fields are rejected, not
    // silently collapsed by an intermediate JSON map.
    let request = serde_json::from_slice(bytes).map_err(|e| ApiFailure::bad(e.to_string()))?;
    let required = E::OPERATION.required_body_fields();
    if !required.is_empty() {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| ApiFailure::bad(e.to_string()))?;
        for field in required {
            if value.get(*field).is_none() {
                return Err(ApiFailure::bad(format!("missing field `{field}`")));
            }
        }
    }
    Ok(request)
}
pub fn parse_query<E: Endpoint>(query: &str) -> Result<E::Query, ApiFailure> {
    if !E::OPERATION.route().has_query {
        return Err(ApiFailure::internal());
    }
    let mut result: E::Query =
        serde_urlencoded::from_str(query).map_err(|e| ApiFailure::bad(e.to_string()))?;
    result.normalize()?;
    Ok(result)
}

#[cfg(test)]
mod tests;
