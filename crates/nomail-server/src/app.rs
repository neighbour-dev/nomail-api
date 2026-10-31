//! Actix transport: Core alone resolves routes and defines request/response DTOs.
use crate::{
    AppState, auth,
    error::{ApiError, ApiResult, success},
    message, webhook,
};
use actix_web::{HttpRequest, HttpResponse, ResponseError, web};
use futures_util::StreamExt;
use nomail_core::api::{
    self, ApiFailure, AuthRequirement, BodyKind, HealthResponse, Operation, endpoints as e,
};
use nomail_core::auth::AuthContext;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    // No local route table or framework-specific path patterns to drift.
    cfg.default_service(web::to(dispatch));
}
async fn dispatch(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Payload,
) -> HttpResponse {
    match execute(state, req, payload).await {
        Ok(response) => response,
        Err(error) => error.error_response(),
    }
}
async fn read_body(payload: &mut web::Payload) -> Result<Vec<u8>, ApiError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|e| ApiError::bad(e.to_string()))?;
        api::check_body_length(bytes.len().saturating_add(chunk.len()))?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
fn actor(context: &mut Option<AuthContext>) -> Result<auth::Auth, ApiError> {
    context
        .take()
        .map(auth::Auth)
        .ok_or_else(|| ApiError::internal("missing authenticated route context"))
}

async fn execute(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: web::Payload,
) -> ApiResult {
    let matched = api::resolve(req.method().as_str(), req.path())?;
    let mut context = if matched.route.auth == AuthRequirement::Bearer {
        let header = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok());
        Some(
            auth::service(&state.db, &state.config)
                .authenticate(api::bearer_token(header)?)
                .await?,
        )
    } else {
        None
    };
    let bytes = if matched.route.body != BodyKind::None {
        read_body(&mut payload).await?
    } else {
        Vec::new()
    };
    let content_type = req
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    macro_rules! body {
        ($endpoint:ty) => {
            web::Json(api::parse_json::<$endpoint>(
                &bytes,
                content_type.as_deref(),
            )?)
        };
    }
    macro_rules! path {
        () => {
            web::Path::from(matched.parameter()?.to_owned())
        };
    }
    // Exhaustive operation binding: a new Core endpoint fails compilation here
    // until a handler is supplied. There are NO method/path literals below.
    match matched.route.operation {
        Operation::Health => Ok(health(state, req).await),
        Operation::Login => {
            let body = body!(e::Login);
            auth::session::login(state, req, body).await
        }
        Operation::Refresh => {
            let body = body!(e::Refresh);
            auth::session::refresh(state, req, body).await
        }
        Operation::Logout => auth::session::logout(state, body!(e::Logout)).await,
        Operation::GetAccount => auth::account::get(state, actor(&mut context)?).await,
        Operation::UpdateAccount => {
            auth::account::update(state, actor(&mut context)?, body!(e::UpdateAccount)).await
        }
        Operation::ListDevices => auth::devices::list(state, actor(&mut context)?).await,
        Operation::KickDevice => auth::devices::kick(state, actor(&mut context)?, path!()).await,
        Operation::DeleteDevice => {
            auth::devices::delete(state, actor(&mut context)?, path!()).await
        }
        Operation::ListConfigs => {
            webhook::configs::list(
                state,
                actor(&mut context)?,
                web::Query(api::parse_query::<e::ListConfigs>(req.query_string())?),
            )
            .await
        }
        Operation::CreateConfig => {
            webhook::configs::create(state, actor(&mut context)?, body!(e::CreateConfig)).await
        }
        Operation::GetConfig => webhook::configs::get(state, actor(&mut context)?, path!()).await,
        Operation::UpdateConfig => {
            webhook::configs::update(state, actor(&mut context)?, path!(), body!(e::UpdateConfig))
                .await
        }
        Operation::DeleteConfig => {
            webhook::configs::delete(state, actor(&mut context)?, path!()).await
        }
        Operation::EnableConfig => {
            webhook::configs::enabled(state, actor(&mut context)?, path!(), body!(e::EnableConfig))
                .await
        }
        Operation::ReceiveWebhook => {
            webhook::receive::handle(state, req, path!(), web::Bytes::from(bytes)).await
        }
        Operation::ListMessages => {
            message::handler::list(
                state,
                actor(&mut context)?,
                web::Query(api::parse_query::<e::ListMessages>(req.query_string())?),
            )
            .await
        }
        Operation::UnreadCount => message::handler::unread_count(state, actor(&mut context)?).await,
        Operation::StreamMessages => message::handler::stream(state, actor(&mut context)?).await,
        Operation::BatchMessages => {
            message::handler::batch(state, actor(&mut context)?, body!(e::BatchMessages)).await
        }
        Operation::GetMessage => message::handler::get(state, actor(&mut context)?, path!()).await,
        Operation::UpdateMessage => {
            message::handler::update(
                state,
                actor(&mut context)?,
                path!(),
                body!(e::UpdateMessage),
            )
            .await
        }
        Operation::DeleteMessage => {
            message::handler::delete(state, actor(&mut context)?, path!()).await
        }
        Operation::RawMessage => message::handler::raw(state, actor(&mut context)?, path!()).await,
    }
}

#[derive(Clone, Default)]
pub struct HealthLimiter(Arc<Mutex<HashMap<Option<IpAddr>, Instant>>>);
impl HealthLimiter {
    fn allow(&self, ip: Option<IpAddr>) -> bool {
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
async fn health(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if !state.health_limiter.allow(req.peer_addr().map(|a| a.ip())) {
        return HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", "2"))
            .json(ApiFailure::new(429, "Too Many Requests").response());
    }
    success::<e::Health>(HealthResponse::database(state.db.ping().await.is_ok()))
}
