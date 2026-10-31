use crate::{
    AppState,
    error::{ApiResult, success},
};
use actix_web::{HttpRequest, web};
use nomail_app::auth::{LoginRequest, RefreshRequest, RequestMeta};
use nomail_app::http::endpoints as e;

pub fn request_meta(req: &HttpRequest) -> RequestMeta {
    // Forwarded headers remain untrusted unless proxy support is configured.
    RequestMeta::new(
        req.peer_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_default(),
        req.headers()
            .get("User-Agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into(),
    )
}
pub async fn login(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LoginRequest>,
) -> ApiResult {
    Ok(success::<e::Login>(
        super::service(&state.db, &state.config)
            .login(body.into_inner(), request_meta(&req))
            .await?,
    ))
}
pub async fn refresh(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<RefreshRequest>,
) -> ApiResult {
    Ok(success::<e::Refresh>(
        super::service(&state.db, &state.config)
            .refresh(body.into_inner(), request_meta(&req))
            .await?,
    ))
}
pub async fn logout(state: web::Data<AppState>, body: web::Json<RefreshRequest>) -> ApiResult {
    super::service(&state.db, &state.config)
        .logout(body.into_inner())
        .await?;
    Ok(success::<e::Logout>(()))
}
