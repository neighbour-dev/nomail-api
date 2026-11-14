use sea_orm::TransactionTrait;
use sea_orm_migration::prelude::*;

pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(Baseline),
            Box::new(MessageCursor),
            Box::new(RawStorage),
            Box::new(MessageSummary),
        ]
    }
}

#[derive(DeriveMigrationName)]
struct Baseline;

struct MessageCursor;

struct RawStorage;
struct MessageSummary;
impl MigrationName for MessageSummary {
    fn name(&self) -> &str {
        "m004_message_summary"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for MessageSummary {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;
        transaction
            .execute_unprepared(include_str!("../migrations/004_message_summary.sql"))
            .await?;
        transaction.commit().await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "destructive storage rollback is disabled".into(),
        ))
    }
}
impl MigrationName for RawStorage {
    fn name(&self) -> &str {
        "m003_raw_storage"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for RawStorage {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;
        transaction
            .execute_unprepared(include_str!("../migrations/003_raw_storage.sql"))
            .await?;
        transaction.commit().await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "destructive storage rollback is disabled".into(),
        ))
    }
}
impl MigrationName for MessageCursor {
    fn name(&self) -> &str {
        "m002_message_cursor"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for MessageCursor {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../migrations/002_message_cursor.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "destructive schema rollback is disabled".into(),
        ))
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Baseline {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("../migrations/001_baseline.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "destructive schema rollback is disabled".into(),
        ))
    }
}
