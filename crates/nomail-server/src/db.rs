use crate::{config::Config, migration::Migrator};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, SqlxSqliteConnector, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::{fmt, path::Path, time::Duration};

/// Database startup stages, separate from request-time storage/domain errors.
pub enum ConnectError {
    Directory(std::io::Error),
    Connection(sqlx::Error),
    Migration(sea_orm::DbErr),
}
impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Directory(_) => "failed to create database directory",
            Self::Connection(_) => "failed to connect to database",
            Self::Migration(_) => "failed to migrate database",
        })
    }
}
impl fmt::Debug for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Directory(error) => error,
            Self::Connection(error) => error,
            Self::Migration(error) => error,
        })
    }
}

pub async fn connect(config: &Config) -> Result<DatabaseConnection, ConnectError> {
    let path = Path::new(&config.database_path);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(ConnectError::Directory)?;
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
        .map_err(ConnectError::Connection)?;
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    Migrator::up(&db, None)
        .await
        .map_err(ConnectError::Migration)?;
    // No legacy configuration migration is provided. Fail at startup rather
    // than silently accepting old single-secret schemas or deleting user data.
    db.execute_unprepared("SELECT encrypted_platform FROM webhook_configs LIMIT 0")
        .await
        .map_err(ConnectError::Migration)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn startup_error_formatting_is_safe_and_retains_explicit_source_access() {
        let error =
            ConnectError::Migration(sea_orm::DbErr::Custom("sensitive database value".into()));
        assert_eq!(error.to_string(), "failed to migrate database");
        assert!(!format!("{error:?}").contains("sensitive"));
        assert!(
            error
                .source()
                .unwrap()
                .to_string()
                .contains("sensitive database value")
        );
    }
}
