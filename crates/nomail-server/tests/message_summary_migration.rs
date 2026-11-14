use nomail_server::migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;

#[tokio::test]
async fn summary_migration_drops_display_columns_without_backfilling_or_clearing_messages() {
    assert_eq!(
        include_str!("../migrations/004_message_summary.sql"),
        include_str!("../../nomail-workers/migrations/0004_message_summary.sql")
    );
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, Some(3)).await.unwrap();
    db.execute_unprepared(r#"
        INSERT INTO webhook_configs(id,created_at,updated_at,user_id,name,key,provider,encrypted_platform)
        VALUES (1,'2026','2026',1,'Kept config','kept','generic','kept-ciphertext');
        INSERT INTO notify_messages(id,created_at,updated_at,webhook_config_id,provider,event_type,title,source,content,level,raw_payload,content_type)
        VALUES (randomblob(16),'2026','2026',1,'generic','message','Kept title','old source','old content','normal','{"source":"native source","content":"native content"}','application/json');
    "#).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT title,summary,raw_payload FROM notify_messages".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<String>("", "title").unwrap(), "Kept title");
    assert_eq!(row.try_get::<String>("", "summary").unwrap(), "");
    assert_eq!(
        row.try_get::<String>("", "raw_payload").unwrap(),
        r#"{"source":"native source","content":"native content"}"#
    );
    assert!(
        db.execute_unprepared("SELECT source FROM notify_messages")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("SELECT content FROM notify_messages")
            .await
            .is_err()
    );
    let config = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT encrypted_platform FROM webhook_configs".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        config.try_get::<String>("", "encrypted_platform").unwrap(),
        "kept-ciphertext"
    );
}
