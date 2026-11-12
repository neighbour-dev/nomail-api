use nomail_server::migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;

#[tokio::test]
async fn raw_schema_upgrade_requires_empty_messages_and_preserves_other_tables() {
    assert_eq!(
        include_str!("../migrations/003_raw_storage.sql"),
        include_str!("../../nomail-workers/migrations/0003_raw_storage.sql")
    );
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, Some(2)).await.unwrap();
    db.execute_unprepared("INSERT INTO users(id,created_at,updated_at,username,password_hash) VALUES (1,'2026','2026','existing-user','unused'); INSERT INTO webhook_configs(id,created_at,updated_at,user_id,name,key,provider,encrypted_platform) VALUES (1,'2026','2026',1,'Existing config','existing-key','generic','existing-ciphertext'); INSERT INTO notify_messages(id,created_at,updated_at,webhook_config_id,provider,event_type,platform_data,title,content,level,raw_payload,content_type) VALUES (randomblob(16),'2026','2026',1,'generic','message','{\"provider\":\"generic\"}','old title','old body','normal','{}','application/json')").await.unwrap();
    let error = Migrator::up(&db, None).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("clear_notify_messages_before_raw_storage"),
        "{error}"
    );
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n, MIN(title) AS title FROM notify_messages",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 1);
    assert_eq!(row.try_get::<String>("", "title").unwrap(), "old title");
    assert!(
        db.execute_unprepared("SELECT platform_data FROM notify_messages LIMIT 0")
            .await
            .is_ok()
    );
    assert!(
        db.execute_unprepared("SELECT raw_headers FROM notify_messages LIMIT 0")
            .await
            .is_err()
    );
    // Operator action in this isolated test database, never an automatic migration.
    db.execute_unprepared("DELETE FROM notify_messages")
        .await
        .unwrap();
    Migrator::up(&db, None).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    assert!(
        db.execute_unprepared(
            "SELECT raw_headers,raw_payload,content_type FROM notify_messages LIMIT 0"
        )
        .await
        .is_ok()
    );
    assert!(
        db.execute_unprepared("SELECT platform_data FROM notify_messages LIMIT 0")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("SELECT * FROM nomail_raw_storage_upgrade_guard")
            .await
            .is_err()
    );
    let user = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT username FROM users WHERE id=1",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        user.try_get::<String>("", "username").unwrap(),
        "existing-user"
    );
    let config = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT encrypted_platform FROM webhook_configs WHERE id=1",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        config.try_get::<String>("", "encrypted_platform").unwrap(),
        "existing-ciphertext"
    );
    db.execute_unprepared("INSERT INTO notify_messages(id,created_at,updated_at,webhook_config_id,provider,event_type,raw_headers,title,content,level,raw_payload,content_type) VALUES (randomblob(16),'2026','2026',1,'generic','message','[]','new title','new body','normal','new body','text/plain')").await.unwrap();
    for value in ["{}", "null", "invalid"] {
        assert!(
            db.execute_unprepared(&format!("UPDATE notify_messages SET raw_headers='{value}'"))
                .await
                .is_err()
        );
    }
    // Once registered, the migration must not run its empty-inbox guard again.
    Migrator::up(&db, None).await.unwrap();
}
