//! HTTP contract, independent of any server framework or transport runtime.
pub mod contract;
mod dispatch;
mod error;
mod services;
pub mod sse;
mod transport;
pub mod webhook;
pub use contract::*;
pub use dispatch::execute;
pub use error::{ApiFailure, HttpError, HttpResult};
pub use services::{Environment, EnvironmentServices, HealthCheck, Services};
pub use transport::{
    HttpOutcome, HttpReply, HttpRequest, HttpResponse, RealtimeEffect, RequestHead, Subscription,
};
#[cfg(feature = "openapi")]
pub mod openapi;

pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
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
    serde_json::from_slice(bytes).map_err(|e| ApiFailure::bad(e.to_string()))
}
pub fn parse_query<E: Endpoint>(query: &str) -> Result<E::Query, ApiFailure> {
    if !E::OPERATION.route().has_query {
        return Err(ApiFailure::internal());
    }
    serde_urlencoded::from_str(query).map_err(|e| ApiFailure::bad(e.to_string()))
}

#[cfg(test)]
mod tests;
