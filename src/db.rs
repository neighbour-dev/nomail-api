use crate::{config::Config, error::ApiError, migration::Migrator};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::time::Duration;

pub async fn connect(config: &Config) -> Result<DatabaseConnection, ApiError> {
    let mut options = ConnectOptions::new(&config.database_url);
    options
        .max_connections(100)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .max_lifetime(Duration::from_secs(3600))
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    Migrator::up(&db, None).await?;
    Ok(db)
}
