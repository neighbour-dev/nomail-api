use sea_orm_migration::prelude::*;

pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(Baseline)]
    }
}

#[derive(DeriveMigrationName)]
struct Baseline;

#[async_trait::async_trait]
impl MigrationTrait for Baseline {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Existing installations already have these tables. The baseline adopts them without
        // altering their data or constraints; fresh databases get the same column names/types.
        manager
            .get_connection()
            .execute_unprepared(include_str!("../migration/001_baseline.sql"))
            .await?;
        Ok(())
    }
    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "baseline rollback is disabled to protect adopted data".into(),
        ))
    }
}
