//! Native authentication storage and local account maintenance.
pub(crate) mod account;
pub(crate) mod repository;

use crate::config::Config;
#[cfg(test)]
use crate::runtime::Runtime;
pub use account::reset_account;
use nomail_app::auth::AuthConfig;
#[cfg(test)]
use nomail_app::auth::AuthService;
#[cfg(test)]
use sea_orm::DatabaseConnection;

pub(crate) fn auth_config(config: &Config) -> AuthConfig {
    AuthConfig {
        jwt_secret: config.jwt_secret.clone(),
        jwt_issuer: config.jwt_issuer.clone(),
    }
}
#[cfg(test)]
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
