use crate::{
    auth,
    config::Config,
    db::Store,
    error::{Result, success},
};
use nomail_app::http::endpoints as e;
use worker::Response;
pub async fn list(uid: i64, db: &Store, config: &Config) -> Result<Response> {
    success::<e::ListDevices>(auth::service(db, config, None).devices(uid).await?)
}
pub async fn kick(identifier: &str, uid: i64, db: &Store, config: &Config) -> Result<Response> {
    auth::service(db, config, None)
        .kick_device(uid, identifier)
        .await?;
    success::<e::KickDevice>(())
}
pub async fn delete(identifier: &str, uid: i64, db: &Store, config: &Config) -> Result<Response> {
    auth::service(db, config, None)
        .delete_device(uid, identifier)
        .await?;
    success::<e::DeleteDevice>(())
}
