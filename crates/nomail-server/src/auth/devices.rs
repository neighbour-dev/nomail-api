use super::Auth;
use crate::{
    AppState,
    error::{ApiResult, success},
};
use actix_web::web;
use nomail_app::http::endpoints as e;

pub async fn list(state: web::Data<AppState>, auth: Auth) -> ApiResult {
    Ok(success::<e::ListDevices>(
        super::service(&state.db, &state.config)
            .devices(auth.user_id)
            .await?,
    ))
}
pub async fn kick(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    super::service(&state.db, &state.config)
        .kick_device(auth.user_id, &path)
        .await?;
    Ok(success::<e::KickDevice>(()))
}
pub async fn delete(state: web::Data<AppState>, auth: Auth, path: web::Path<String>) -> ApiResult {
    super::service(&state.db, &state.config)
        .delete_device(auth.user_id, &path)
        .await?;
    Ok(success::<e::DeleteDevice>(()))
}
