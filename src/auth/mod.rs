mod account;
pub(crate) mod devices;
mod session;

use crate::{
    AppState,
    entity::{device, session as session_entity},
    error::ApiError,
};
pub use account::{ensure_initial_user, reset_account};
use actix_web::{FromRequest, HttpRequest, dev::Payload, web};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};

pub const ACCESS_TTL: i64 = 2 * 60 * 60;
pub const REFRESH_DAYS: i64 = 7;
pub const MAX_DEVICES: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: i64,
    pub session_id: i64,
    pub sub: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
}

pub fn access_token(
    state: &AppState,
    user_id: i64,
    session_id: i64,
) -> Result<(String, DateTime<Utc>), ApiError> {
    let now = Utc::now();
    let expires = now + Duration::seconds(ACCESS_TTL);
    let claims = Claims {
        user_id,
        session_id,
        sub: user_id.to_string(),
        iss: state.config.jwt_issuer.clone(),
        iat: now.timestamp(),
        exp: expires.timestamp(),
    };
    let token = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(ApiError::internal)?;
    Ok((token, expires))
}

pub fn decode_token(token: &str, state: &AppState) -> Result<Claims, ApiError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[&state.config.jwt_issuer]);
    validation.leeway = 0;
    let claims = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::unauthorized())?
    .claims;
    if claims.user_id <= 0 || claims.session_id <= 0 {
        return Err(ApiError::unauthorized());
    }
    Ok(claims)
}

pub struct Auth {
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
}

impl FromRequest for Auth {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let state = req.app_data::<web::Data<AppState>>().cloned();
        let token = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_owned);
        Box::pin(async move {
            let state = state.ok_or_else(|| ApiError::internal("missing application state"))?;
            let claims =
                decode_token(token.as_deref().ok_or_else(ApiError::unauthorized)?, &state)?;
            let session = session_entity::Entity::find_by_id(claims.session_id)
                .filter(session_entity::Column::DeletedAt.is_null())
                .filter(session_entity::Column::RevokedAt.is_null())
                .filter(session_entity::Column::ExpiresAt.gt(Utc::now()))
                .one(&state.db)
                .await?
                .ok_or_else(ApiError::unauthorized)?;
            let device = device::Entity::find_by_id(session.device_id)
                .filter(device::Column::UserId.eq(claims.user_id))
                .filter(device::Column::DeletedAt.is_null())
                .one(&state.db)
                .await?
                .ok_or_else(ApiError::unauthorized)?;
            let now = Utc::now();
            if device
                .last_seen_at
                .is_none_or(|seen| seen < now - Duration::minutes(5))
            {
                let mut active: device::ActiveModel = device.into();
                active.last_seen_at = Set(Some(now));
                active.updated_at = Set(now);
                active.update(&state.db).await?;
            }
            Ok(Self {
                user_id: claims.user_id,
                expires_at: DateTime::from_timestamp(claims.exp, 0)
                    .ok_or_else(ApiError::unauthorized)?,
            })
        })
    }
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/login", web::post().to(session::login))
            .route("/refresh-token", web::post().to(session::refresh))
            .route("/logout", web::post().to(session::logout))
            .route("/account", web::get().to(account::get))
            .route("/account", web::patch().to(account::update))
            .route("/devices", web::get().to(devices::list))
            .route(
                "/devices/{device_id}/session",
                web::delete().to(devices::kick),
            )
            .route("/devices/{device_id}", web::delete().to(devices::delete)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> AppState {
        AppState {
            db: sea_orm::DatabaseConnection::Disconnected,
            config: crate::config::Config {
                database_url: String::new(),
                jwt_secret: "test-secret".into(),
                jwt_issuer: "nomail-test".into(),
                encryption_key: [0; 32],
                port: 3000,
                initial_username: String::new(),
                initial_password: String::new(),
            },
            realtime: crate::realtime::Hub::default(),
            health_limiter: crate::app::HealthLimiter::default(),
        }
    }
    #[test]
    fn jwt_claims_expiration_issuer_and_session() {
        let state = state();
        let (token, _) = access_token(&state, 7, 11).unwrap();
        let mut claims = decode_token(&token, &state).unwrap();
        assert_eq!((claims.user_id, claims.session_id), (7, 11));
        let sign = |claims: &Claims| {
            jsonwebtoken::encode(
                &Header::new(Algorithm::HS256),
                claims,
                &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            )
            .unwrap()
        };
        claims.session_id = 0;
        assert!(decode_token(&sign(&claims), &state).is_err());
        claims.session_id = 11;
        claims.iss = "wrong-issuer".into();
        assert!(decode_token(&sign(&claims), &state).is_err());
        claims.iss = "nomail-test".into();
        claims.exp = Utc::now().timestamp() - 1;
        assert!(decode_token(&sign(&claims), &state).is_err());
        assert!(decode_token("not-a-token", &state).is_err());
    }
}
