use nomail_server::migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;

#[tokio::test]
async fn cursor_migration_upgrades_existing_data_and_supports_indexed_seek_in_both_directions() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    // Stop at the original baseline, simulating an installation before cursors.
    Migrator::up(&db, Some(1)).await.unwrap();
    db.execute_unprepared("INSERT INTO webhook_configs(id,created_at,updated_at,user_id,name,key,provider,encrypted_platform) VALUES (1,'2026','2026',1,'A','a','generic','dummy'),(2,'2026','2026',1,'B','b','generic','dummy')").await.unwrap();
    db.execute_unprepared(r#"WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<5000)
        INSERT INTO notify_messages(id,created_at,updated_at,webhook_config_id,provider,event_type,platform_data,title,content,level,raw_payload,content_type)
        SELECT randomblob(16),printf('2040-01-01T00:%02d:%02d.123+00:00',i/600,(i/10)%60),'2026',i%2+1,'generic','message','{"provider":"generic"}','title','body','normal','{}','application/json' FROM n"#).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    let count = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM notify_messages".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count.try_get::<i64>("", "n").unwrap(), 5000);
    db.execute_unprepared("ANALYZE").await.unwrap();
    for (extra, expected_index) in [
        ("", "idx_messages_active_time_id"),
        (
            " AND webhook_config_id=1",
            "idx_messages_active_config_time_id",
        ),
    ] {
        for (operator, order) in [("<", "DESC"), (">", "ASC")] {
            let sql = format!(
                "EXPLAIN QUERY PLAN SELECT id,title,content FROM notify_messages WHERE deleted_at IS NULL AND webhook_config_id IN (SELECT id FROM webhook_configs WHERE user_id=1 AND deleted_at IS NULL){extra} AND (created_at,id){operator}('2040-01-01T00:04:00.123+00:00',zeroblob(16)) ORDER BY created_at {order},id {order} LIMIT 21"
            );
            let rows = db
                .query_all(Statement::from_string(DbBackend::Sqlite, sql))
                .await
                .unwrap();
            let plan = rows
                .iter()
                .map(|row| row.try_get::<String>("", "detail").unwrap())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(plan.contains(expected_index), "{plan}");
            assert!(!plan.contains("TEMP B-TREE"), "{plan}");
        }
    }
}
