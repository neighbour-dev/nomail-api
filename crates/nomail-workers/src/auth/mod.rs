//! Workers request extraction and composition. All auth use cases are in App.
mod repository;
mod runtime;
use crate::{
    config::Config,
    db::Store,
    error::{ApiError, Result, success},
};
use nomail_app::auth::{AuthConfig, AuthContext, AuthService, RequestMeta};
use nomail_app::http::endpoints as e;
use worker::{Env, Request, Response};

pub(crate) fn service<'a>(
    db: &'a Store,
    config: &Config,
    env: Option<&Env>,
) -> AuthService<repository::Store<'a>, runtime::Runtime> {
    AuthService {
        store: repository::Store(db),
        runtime: runtime::Runtime,
        config: AuthConfig {
            jwt_secret: config.jwt_secret.clone(),
            jwt_issuer: config.jwt_issuer.clone(),
            initial_username: env
                .and_then(|env| env.secret("INITIAL_USERNAME").ok())
                .map(|v| v.to_string())
                .unwrap_or_default(),
            initial_password: env
                .and_then(|env| env.secret("INITIAL_PASSWORD").ok())
                .map(|v| v.to_string())
                .unwrap_or_default(),
        },
    }
}
fn request_meta(req: &Request) -> Result<RequestMeta> {
    Ok(RequestMeta::new(
        req.headers().get("CF-Connecting-IP")?.unwrap_or_default(),
        req.headers().get("User-Agent")?.unwrap_or_default(),
    ))
}
pub async fn authenticate(req: &Request, db: &Store, config: &Config) -> Result<AuthContext> {
    let header = req
        .headers()
        .get("Authorization")?
        .ok_or_else(ApiError::unauthorized)?;
    let token = nomail_app::http::bearer_token(Some(&header))?;
    Ok(service(db, config, None).authenticate(token).await?)
}
pub async fn login(
    req: &Request,
    body: nomail_app::auth::LoginRequest,
    db: &Store,
    config: &Config,
    env: &Env,
) -> Result<Response> {
    success::<e::Login>(
        service(db, config, Some(env))
            .login(body, request_meta(req)?)
            .await?,
    )
}
pub async fn refresh(
    req: &Request,
    body: nomail_app::auth::RefreshRequest,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    success::<e::Refresh>(
        service(db, config, None)
            .refresh(body, request_meta(req)?)
            .await?,
    )
}
pub async fn logout(
    body: nomail_app::auth::RefreshRequest,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    service(db, config, None).logout(body).await?;
    success::<e::Logout>(())
}
pub async fn get_account(uid: i64, db: &Store, config: &Config) -> Result<Response> {
    let service = service(db, config, None);
    success::<e::GetAccount>(service.account(uid).await?)
}
pub async fn update_account(
    body: nomail_app::auth::AccountUpdateRequest,
    uid: i64,
    db: &Store,
    config: &Config,
) -> Result<Response> {
    success::<e::UpdateAccount>(service(db, config, None).update_account(uid, body).await?)
}
