//! The platform boundary contains transport capabilities, never API operations.
use super::{ApiFailure, ApiResponse, Endpoint, HttpResult};
use crate::{
    auth::RequestMeta,
    webhook::{HeaderLookup, RawHeader},
};
use chrono::{DateTime, Utc};

pub struct RequestHead {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    /// Complete runtime-visible bytes, separate from text authentication lookup.
    pub raw_headers: Option<Vec<RawHeader>>,
    pub metadata: RequestMeta,
}
impl HeaderLookup for RequestHead {
    fn entries(&self) -> Vec<RawHeader> {
        self.raw_headers
            .clone()
            .unwrap_or_else(|| self.headers.entries())
    }
    fn values(&self, name: &str) -> Vec<&str> {
        self.headers.values(name)
    }
}

#[allow(async_fn_in_trait)]
pub trait HttpRequest {
    fn head(&self) -> &RequestHead;
    /// Called only after routing/authentication. Adapters enforce MAX_BODY_BYTES
    /// incrementally; execute also checks the completed body defensively.
    async fn read_body(&mut self) -> HttpResult<Vec<u8>>;
}

pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
pub struct Subscription {
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
}
pub enum HttpReply {
    Response(HttpResponse),
    Subscribe(Subscription),
}
pub struct RealtimeEffect {
    pub user_id: i64,
    pub event: super::sse::MessageCreated,
}
pub struct HttpOutcome {
    pub reply: HttpReply,
    pub events: Vec<RealtimeEffect>,
}
impl HttpOutcome {
    pub fn response(status: u16, content_type: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            reply: HttpReply::Response(HttpResponse {
                status,
                headers: vec![("Content-Type".into(), content_type.into())],
                body,
            }),
            events: Vec::new(),
        }
    }
    pub fn subscribe(user_id: i64, expires_at: DateTime<Utc>) -> Self {
        Self {
            reply: HttpReply::Subscribe(Subscription {
                user_id,
                expires_at,
            }),
            events: Vec::new(),
        }
    }
}
pub(super) fn json<E: Endpoint>(data: E::Response) -> HttpResult<HttpOutcome> {
    let body =
        serde_json::to_vec(&ApiResponse::success(data)).map_err(|_| ApiFailure::internal())?;
    Ok(HttpOutcome::response(200, "application/json", body))
}
