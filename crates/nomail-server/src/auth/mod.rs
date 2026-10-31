//! Native authentication storage and local account maintenance.
pub(crate) mod account;
pub(crate) mod repository;

use crate::{config::Config, runtime::Runtime};
pub use account::{ensure_initial_user, reset_account};
use nomail_app::auth::{AuthConfig, AuthService};
use sea_orm::DatabaseConnection;

pub(crate) fn auth_config(config: &Config) -> AuthConfig {
    AuthConfig {
        jwt_secret: config.jwt_secret.clone(),
        jwt_issuer: config.jwt_issuer.clone(),
        initial_username: config.initial_username.clone(),
        initial_password: config.initial_password.clone(),
    }
}
pub(crate) fn service<'a>(
    db: &'a DatabaseConnection,
    config: &Config,
) -> AuthService<repository::Store<'a>, Runtime> {
    AuthService {
        store: repository::Store(db),
        runtime: Runtime {
            encryption_key: config.encryption_key,
        },
        config: auth_config(config),
    }
}
