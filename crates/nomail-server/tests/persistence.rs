use chrono::{Duration, Utc};
use nomail_server::{
    auth,
    config::Config,
    crypto, db,
    entity::{device, session, user},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Set,
};

fn config(directory: &tempfile::TempDir) -> Config {
    Config {
        database_path: directory
            .path()
            .join("data/nomail.db")
            .to_str()
            .unwrap()
            .into(),
        jwt_secret: "test-only-secret".into(),
        jwt_issuer: "nomail-test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    }
}

#[tokio::test]
async fn file_persistence_wal_foreign_keys_and_atomic_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let config = config(&directory);
    let first = db::connect(&config).await.unwrap();
    let second = db::connect(&config).await.unwrap();
    let (a, b) = tokio::join!(
        auth::ensure_initial_user(&first, &config),
        auth::ensure_initial_user(&second, &config)
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(user::Entity::find().count(&first).await.unwrap(), 1);
    // Check each physical pool connection, not just one borrowed connection.
    let pool = first.get_sqlite_connection_pool();
    let mut connections = Vec::new();
    for _ in 0..4 {
        let mut connection = pool.acquire().await.unwrap();
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(journal, "wal");
        connections.push(connection);
    }
    drop(connections);
    first.close().await.unwrap();
    second.close().await.unwrap();
    let reopened = db::connect(&config).await.unwrap();
    assert_eq!(
        user::Entity::find()
            .one(&reopened)
            .await
            .unwrap()
            .unwrap()
            .username,
        "admin"
    );
    assert!(reopened.execute_unprepared("INSERT INTO devices (user_id,device_id,platform,device_type,created_at,updated_at) VALUES (999,'invalid','ios','phone','2026-01-01','2026-01-01')").await.is_err());
    reopened.close().await.unwrap();
}

#[tokio::test]
async fn cancellation_rolls_back_and_reset_revokes_sessions() {
    let directory = tempfile::tempdir().unwrap();
    let config = config(&directory);
    let database = db::connect(&config).await.unwrap();
    auth::ensure_initial_user(&database, &config).await.unwrap();
    {
        let tx = db::write_transaction(&database).await.unwrap();
        tx.execute_unprepared("UPDATE users SET username='not-committed'")
            .await
            .unwrap();
        // Dropping a live transaction must roll back, including its write lock.
    }
    let now = Utc::now();
    let d = device::ActiveModel {
        user_id: Set(1),
        device_id: Set("a".into()),
        platform: Set("ios".into()),
        device_type: Set("phone".into()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    session::ActiveModel {
        device_id: Set(d.id),
        refresh_token_hash: Set(crypto::hash_token("refresh")),
        expires_at: Set(now + Duration::days(7)),
        logged_in_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&database)
    .await
    .unwrap();
    auth::reset_account(&database, None, "newpassword123".into())
        .await
        .unwrap();
    let account = user::Entity::find().one(&database).await.unwrap().unwrap();
    assert_eq!(account.username, "admin");
    crypto::verify_password("newpassword123".into(), account.password_hash)
        .await
        .unwrap();
    assert_eq!(
        session::Entity::find()
            .filter(session::Column::RevokedAt.is_null())
            .count(&database)
            .await
            .unwrap(),
        0
    );
    database.close().await.unwrap();
}
