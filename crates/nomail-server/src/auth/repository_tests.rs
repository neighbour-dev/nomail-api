use super::*;
use crate::{auth::runtime::Runtime, config::Config};

fn config(dir: &tempfile::TempDir) -> Config {
    Config {
        database_path: dir.path().join("auth.db").to_str().unwrap().into(),
        jwt_secret: "test-secret".into(),
        jwt_issuer: "test".into(),
        encryption_key: [7; 32],
        port: 3000,
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    }
}
fn login_request(id: &str) -> Login {
    Login {
        username: "admin".into(),
        password: "password123".into(),
        device: DeviceInput {
            device_id: id.into(),
            platform: "ios".into(),
            device_type: "phone".into(),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn stale_verified_credentials_cannot_create_a_device_or_change_the_account() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(&dir);
    let db = crate::db::connect(&config).await.unwrap();
    let service = crate::auth::service(&db, &config);
    service.ensure_initial_account().await.unwrap();
    let before = service.store.account_by_id(1).await.unwrap().unwrap();
    service
        .reset_account(None, "changedpassword123")
        .await
        .unwrap();
    let result = service
        .store
        .complete_login(LoginWrite {
            user_id: 1,
            expected_password_hash: before.password_hash.clone(),
            device: login_request("stale").device,
            meta: RequestMeta::default(),
            refresh_token_hash: "stale-refresh-hash".into(),
            expires_at: Utc::now() + chrono::Duration::days(7),
            now: Utc::now(),
            max_devices: MAX_DEVICES,
        })
        .await;
    assert!(matches!(result, Err(AuthError::Unauthorized)));
    assert!(service.store.devices(1).await.unwrap().is_empty());
    assert!(matches!(
        service
            .store
            .update_account(AccountWrite {
                user_id: 1,
                expected_password_hash: before.password_hash,
                username: "must-not-change".into(),
                new_password_hash: None,
                now: Utc::now()
            })
            .await,
        Err(AuthError::Unauthorized)
    ));
    assert_eq!(
        service
            .store
            .account_by_id(1)
            .await
            .unwrap()
            .unwrap()
            .username,
        "admin"
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn password_change_revokes_expired_sessions_too_and_device_actions_are_owned() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(&dir);
    let db = crate::db::connect(&config).await.unwrap();
    let service = crate::auth::service(&db, &config);
    service
        .login(login_request("expired"), RequestMeta::default())
        .await
        .unwrap();
    session::Entity::update_many()
        .col_expr(
            session::Column::ExpiresAt,
            Expr::value(Utc::now() - chrono::Duration::days(1)),
        )
        .exec(&db)
        .await
        .unwrap();
    let login = service
        .login(login_request("active"), RequestMeta::default())
        .await
        .unwrap();
    assert!(matches!(
        service.store.delete_device(999, "active").await,
        Err(AuthError::NotFound(_))
    ));
    assert!(matches!(
        service
            .store
            .revoke_device_session(999, "active", Utc::now())
            .await,
        Err(AuthError::NotFound(_))
    ));
    service.authenticate(&login.access_token).await.unwrap();
    service
        .update_account(
            1,
            UpdateAccount {
                current_password: "password123".into(),
                username: String::new(),
                new_password: "newpassword123".into(),
            },
        )
        .await
        .unwrap();
    let sessions = session::Entity::find().all(&db).await.unwrap();
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().all(|s| s.revoked_at.is_some()));
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    // Runtime calls the common password implementation, not a second policy.
    assert!(
        nomail_app::auth::AuthRuntime::verify_password(
            &Runtime,
            "newpassword123",
            &service
                .store
                .account_by_id(1)
                .await
                .unwrap()
                .unwrap()
                .password_hash
        )
        .await
        .unwrap()
    );
    db.close().await.unwrap();
}
