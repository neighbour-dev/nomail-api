use crate::{config::Config, error::ApiError, migration::Migrator};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, SqlxSqliteConnector, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::{path::Path, time::Duration};

pub async fn connect(config: &Config) -> Result<DatabaseConnection, ApiError> {
    let path = Path::new(&config.database_path);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(ApiError::internal)?;
    }
    // Connection options are applied to EVERY connection, not just the first pool member.
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(Duration::from_secs(10));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .min_connections(1)
        .connect_with(options)
        .await
        .map_err(ApiError::internal)?;
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    Migrator::up(&db, None).await?;
    Ok(db)
}

/// Acquire SQLite's database write reservation BEFORE any decision-making reads.
/// The first UPDATE starts a write transaction, avoiding deferred read-to-write
/// upgrades and serializing writers across pools/processes, not just this runtime.
/// SeaORM owns rollback/commit, including cancellation rollback.
pub async fn write_transaction(
    db: &DatabaseConnection,
) -> Result<DatabaseTransaction, sea_orm::DbErr> {
    let tx = db.begin().await?;
    tx.execute_unprepared("UPDATE nomail_write_lock SET id = id WHERE id = 1")
        .await?;
    Ok(tx)
}
