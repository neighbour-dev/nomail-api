use crate::config::Config;
use nomail_app::auth::AuthResult;
use sea_orm::DatabaseConnection;

pub async fn ensure_initial_user(db: &DatabaseConnection, config: &Config) -> AuthResult<()> {
    super::service(db, config).ensure_initial_account().await
}
pub async fn reset_account(
    db: &DatabaseConnection,
    username: Option<String>,
    password: String,
) -> AuthResult<()> {
    // Reset needs no token-signing configuration and remains a local-only tool.
    let config = nomail_app::auth::AuthConfig {
        jwt_secret: String::new(),
        jwt_issuer: String::new(),
        initial_username: String::new(),
        initial_password: String::new(),
    };
    nomail_app::auth::AuthService {
        store: super::repository::Store(db),
        runtime: super::runtime::Runtime,
        config,
    }
    .reset_account(username, &password)
    .await
}
