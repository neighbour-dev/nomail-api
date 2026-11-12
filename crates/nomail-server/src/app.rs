//! Actix request/response adaptation. All API execution lives in nomail-app::http.
use crate::{AppState, error::ApiResult, realtime, services::NativeEnvironment};
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use futures_util::StreamExt;
use nomail_app::{
    auth::RequestMeta,
    http::{self, ApiFailure, HttpReply, HttpResult, RequestHead},
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
        let headers = req
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_owned()))
            })
            .collect();
        let raw_headers = Some(
            req.headers()
                .iter()
                .map(|(name, value)| {
                    nomail_app::webhook::RawHeader::from_bytes(name.as_str(), value.as_bytes())
                })
                .collect(),
        );
        Self {
            head: RequestHead {
                method: req.method().as_str().into(),
                path: req.path().into(),
                query: req.query_string().into(),
                headers,
                raw_headers,
                metadata: RequestMeta::new(
                    crate::client_ip::resolve(&req).to_string(),
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
        type Runtime = crate::runtime::Runtime;
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
                raw_headers: None,
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
            .append_header(("X-Empty", ""))
            .append_header(("X-Tab", "one\ttwo"))
            .insert_header(("X-Forwarded-For", "untrusted"))
            .to_http_parts();
        let payload = <web::Payload as actix_web::FromRequest>::from_request(&req, &mut payload)
            .await
            .unwrap();
        let request = ActixRequest::new(req, payload);
        assert_eq!(request.head.values("X-Repeated"), ["one", "two"]);
        let raw = request.head.entries();
        assert_eq!(
            raw.iter()
                .filter(|h| h.name == "x-repeated")
                .map(|h| h.value.as_str())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert!(
            raw.iter()
                .any(|h| h.name == "x-empty" && h.value.is_empty())
        );
        assert_eq!(request.head.values("X-Tab"), ["one\ttwo"]);
        assert_eq!(request.head.metadata.ip, "127.0.0.1");
    }

    #[actix_web::test]
    async fn request_adapter_preserves_opaque_headers_without_using_them_for_authentication() {
        for name in ["X-NoMail-Title", "X-Unknown", "Authorization"] {
            // DEL/control bytes are already rejected by Actix's HeaderValue.
            for value in ["中文".as_bytes(), &[0xff]] {
                let (req, mut payload) = actix_web::test::TestRequest::get()
                    .uri("/api/health")
                    .append_header((
                        name,
                        actix_web::http::header::HeaderValue::from_bytes(value).unwrap(),
                    ))
                    .to_http_parts();
                let payload =
                    <web::Payload as actix_web::FromRequest>::from_request(&req, &mut payload)
                        .await
                        .unwrap();
                let request = ActixRequest::new(req, payload);
                assert!(request.head.values(name).is_empty());
                let expected =
                    nomail_app::webhook::RawHeader::from_bytes(name.to_ascii_lowercase(), value);
                assert!(request.head.entries().contains(&expected));
            }
        }
    }

    #[actix_web::test]
    async fn unrelated_non_ascii_header_does_not_block_health() {
        let environment = UnconfiguredEnvironment {
            service_calls: std::cell::Cell::new(0),
        };
        for (name, value) in [("X-NoMail-Title", "中文"), ("Authorization", "Bearer é")] {
            let mut request = UnreadableRequest {
                head: RequestHead {
                    method: "GET".into(),
                    path: "/api/health".into(),
                    query: String::new(),
                    headers: vec![(name.into(), value.into())],
                    raw_headers: None,
                    metadata: RequestMeta::default(),
                },
                body_reads: 0,
            };
            let outcome = http::execute(&mut request, &environment).await.unwrap();
            let HttpReply::Response(response) = outcome.reply else {
                panic!()
            };
            assert_eq!(response.status, 200);
            assert_eq!(request.body_reads, 0);
        }
        assert_eq!(environment.service_calls.get(), 0);
    }
}
