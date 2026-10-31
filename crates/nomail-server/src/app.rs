//! Actix request/response adaptation. All API execution lives in nomail-app::http.
use crate::{AppState, error::ApiResult, realtime, services::NativeEnvironment};
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use futures_util::StreamExt;
use nomail_app::{
    auth::RequestMeta,
    http::{self, ApiFailure, HttpReply, HttpResult, RequestHead},
};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub fn configure(config: &mut web::ServiceConfig) {
    config.default_service(web::to(handle));
}
async fn handle(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Payload,
) -> HttpResponse {
    match execute(&state, req, payload).await {
        Ok(response) => response,
        Err(error) => error.error_response(),
    }
}
async fn execute(state: &AppState, req: HttpRequest, payload: web::Payload) -> ApiResult {
    let mut request = ActixRequest::new(req, payload);
    let outcome = http::execute(&mut request, &NativeEnvironment(state)).await?;
    for effect in outcome.events {
        state.realtime.publish(
            effect.user_id,
            realtime::Event::MessageCreated(effect.event),
        );
    }
    match outcome.reply {
        HttpReply::Response(response) => {
            let status = actix_web::http::StatusCode::from_u16(response.status)
                .map_err(|_| ApiFailure::internal())?;
            let mut builder = HttpResponse::build(status);
            for (name, value) in response.headers {
                builder.insert_header((name, value));
            }
            Ok(builder.body(response.body))
        }
        HttpReply::Subscribe(subscription) => Ok(realtime::stream(&state.realtime, subscription)),
    }
}

struct ActixRequest {
    head: RequestHead,
    payload: web::Payload,
}
impl ActixRequest {
    fn new(req: HttpRequest, payload: web::Payload) -> Self {
        Self {
            head: RequestHead {
                method: req.method().as_str().into(),
                path: req.path().into(),
                query: req.query_string().into(),
                headers: req
                    .headers()
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.to_string(), value.to_owned()))
                    })
                    .collect(),
                metadata: RequestMeta::new(
                    // Forwarded headers are not trusted on the self-hosted server.
                    req.peer_addr()
                        .map(|a| a.ip().to_string())
                        .unwrap_or_default(),
                    req.headers()
                        .get("User-Agent")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .into(),
                ),
            },
            payload,
        }
    }
}
impl http::HttpRequest for ActixRequest {
    fn head(&self) -> &RequestHead {
        &self.head
    }
    async fn read_body(&mut self) -> HttpResult<Vec<u8>> {
        let mut bytes = Vec::new();
        while let Some(chunk) = self.payload.next().await {
            let chunk = chunk.map_err(|error| ApiFailure::bad(error.to_string()))?;
            http::check_body_length(bytes.len().saturating_add(chunk.len()))?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

#[derive(Clone, Default)]
pub struct HealthLimiter(Arc<Mutex<HashMap<Option<IpAddr>, Instant>>>);
impl HealthLimiter {
    pub(crate) fn allow(&self, ip: Option<IpAddr>) -> bool {
        let now = Instant::now();
        let mut entries = self.0.lock().expect("health limiter lock");
        entries.retain(|_, time| now.duration_since(*time) < Duration::from_secs(2));
        if entries.contains_key(&ip) {
            return false;
        }
        entries.insert(ip, now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomail_app::webhook::HeaderLookup;

    struct UnconfiguredEnvironment {
        service_calls: std::cell::Cell<usize>,
    }
    impl http::Environment for UnconfiguredEnvironment {
        // These are only type witnesses: this environment never constructs a
        // database or any use case, and always fails configuration if called.
        type AuthStore = crate::auth::repository::Store<'static>;
        type MessageStore = crate::message::repository::Store<'static>;
        type ConfigStore = crate::webhook::configs::repository::Store<'static>;
        type IngestionStore = crate::message::repository::Store<'static>;
        type Runtime = crate::services::Runtime;
        fn services(&self) -> HttpResult<http::EnvironmentServices<Self>> {
            self.service_calls.set(self.service_calls.get() + 1);
            Err(ApiFailure::internal().into())
        }
        async fn health(&self, _: &RequestHead) -> HttpResult<http::HealthCheck> {
            Ok(http::HealthCheck::Database(true))
        }
    }
    struct UnreadableRequest {
        head: RequestHead,
        body_reads: usize,
    }
    impl http::HttpRequest for UnreadableRequest {
        fn head(&self) -> &RequestHead {
            &self.head
        }
        async fn read_body(&mut self) -> HttpResult<Vec<u8>> {
            self.body_reads += 1;
            Err(ApiFailure::bad("body must not be read").into())
        }
    }
    #[actix_web::test]
    async fn shared_executor_routes_and_handles_health_before_loading_services_or_body() {
        let environment = UnconfiguredEnvironment {
            service_calls: std::cell::Cell::new(0),
        };
        let mut request = UnreadableRequest {
            head: RequestHead {
                method: "GET".into(),
                path: "/api/unknown".into(),
                query: String::new(),
                headers: Vec::new(),
                metadata: RequestMeta::default(),
            },
            body_reads: 0,
        };
        let error = http::execute(&mut request, &environment)
            .await
            .err()
            .unwrap();
        assert_eq!(error.failure.status, 404);
        assert_eq!(environment.service_calls.get(), 0);

        request.head.path = "/api/health".into();
        let outcome = http::execute(&mut request, &environment).await.unwrap();
        let HttpReply::Response(response) = outcome.reply else {
            panic!()
        };
        assert_eq!(response.status, 200);
        assert_eq!(environment.service_calls.get(), 0);

        request.head.method = "POST".into();
        request.head.path = "/api/auth/login".into();
        let error = http::execute(&mut request, &environment)
            .await
            .err()
            .unwrap();
        assert_eq!(error.failure.status, 500);
        assert_eq!(environment.service_calls.get(), 1);
        assert_eq!(request.body_reads, 0);
    }
    #[actix_web::test]
    async fn request_adapter_preserves_repeated_headers_and_uses_peer_ip() {
        let (req, mut payload) = actix_web::test::TestRequest::get()
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .append_header(("X-Repeated", "one"))
            .append_header(("X-Repeated", "two"))
            .insert_header(("X-Forwarded-For", "untrusted"))
            .to_http_parts();
        let payload = <web::Payload as actix_web::FromRequest>::from_request(&req, &mut payload)
            .await
            .unwrap();
        let request = ActixRequest::new(req, payload);
        assert_eq!(request.head.values("X-Repeated"), ["one", "two"]);
        assert_eq!(request.head.metadata.ip, "127.0.0.1");
    }
}
