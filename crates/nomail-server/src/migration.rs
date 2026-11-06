use sea_orm_migration::prelude::*;

pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(Baseline), Box::new(MessageCursor)]
    }
}

#[derive(DeriveMigrationName)]
struct Baseline;

struct MessageCursor;
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
