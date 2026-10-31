//! Workers request/response adaptation; no endpoint names or business dispatch.
use crate::{
    db::Store,
    error::{ApiError, Result},
    realtime,
    services::WorkersEnvironment,
};
use futures_util::StreamExt;
use nomail_app::{
    auth::RequestMeta,
    http::{self, HttpReply, HttpResult, RequestHead},
};
use worker::{Env, Request, Response};

pub async fn handle(req: Request, env: Env) -> Result<Response> {
    let mut request = WorkersRequest::new(req)?;
    // Keep binding failures deferred until the shared executor has matched a
    // route, so unknown paths still return 404 even with broken configuration.
    let db = env.d1("DB").map(Store);
    let outcome = http::execute(&mut request, &WorkersEnvironment { db: &db, env: &env }).await?;
    for effect in outcome.events {
        if let Err(_error) = realtime::publish(&env, effect.user_id, effect.event).await {
            // A failed best-effort broadcast must not fail an already durable ACK.
            worker::console_error!("realtime publish failed: internal operation failed");
        }
    }
    match outcome.reply {
        HttpReply::Response(response) => {
            let mut result = Response::from_bytes(response.body)?.with_status(response.status);
            for (name, value) in response.headers {
                result.headers_mut().set(&name, &value)?;
            }
            Ok(result)
        }
        HttpReply::Subscribe(subscription) => Ok(realtime::subscribe(
            &env,
            subscription.user_id,
            subscription.expires_at.timestamp(),
        )
        .await?),
    }
}

struct WorkersRequest {
    head: RequestHead,
    request: Request,
}
impl WorkersRequest {
    fn new(request: Request) -> Result<Self> {
        Ok(Self {
            head: RequestHead {
                method: request.method().as_ref().into(),
                path: request.path(),
                query: request.url()?.query().unwrap_or("").into(),
                headers: request.headers().entries().collect(),
                metadata: RequestMeta::new(
                    request
                        .headers()
                        .get("CF-Connecting-IP")?
                        .unwrap_or_default(),
                    request.headers().get("User-Agent")?.unwrap_or_default(),
                ),
            },
            request,
        })
    }
}
impl http::HttpRequest for WorkersRequest {
    fn head(&self) -> &RequestHead {
        &self.head
    }
    async fn read_body(&mut self) -> HttpResult<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut stream = self
            .request
            .stream()
            .map_err(|error| ApiError::from(error).into_http())?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| ApiError::from(error).into_http())?;
            http::check_body_length(bytes.len().saturating_add(chunk.len()))?;
            bytes.extend(chunk);
        }
        Ok(bytes)
    }
}
