use super::*;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use std::cell::{Cell, RefCell};

fn config() -> AuthConfig {
    AuthConfig {
        jwt_secret: "test-shared-auth-secret".into(),
        jwt_issuer: "nomail-test".into(),
        initial_username: "admin".into(),
        initial_password: "password123".into(),
    }
}
fn request(username: &str, password: &str) -> LoginRequest {
    serde_json::from_value(json!({"username":username,"password":password,"deviceId":"phone","platform":"ios","deviceType":"phone"})).unwrap()
}
struct Runtime {
    seconds: Cell<i64>,
    sequence: Cell<u8>,
}
impl Runtime {
    fn new() -> Self {
        Self {
            seconds: Cell::new(1000),
            sequence: Cell::new(0),
        }
    }
}
impl AuthRuntime for Runtime {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.seconds.get(), 0).unwrap()
    }
    fn random_bytes(&self, out: &mut [u8]) -> AuthResult<()> {
        self.sequence.set(self.sequence.get() + 1);
        out.fill(self.sequence.get());
        Ok(())
    }
    // Test doubles execute instantly; real bcrypt policy is tested separately.
    async fn hash_password(&self, value: &str, _salt: [u8; 16]) -> AuthResult<String> {
        Ok(format!("fake:{value}"))
    }
    async fn verify_password(&self, value: &str, hash: &str) -> AuthResult<bool> {
        Ok(hash == format!("fake:{value}"))
    }
}

#[derive(Default)]
struct State {
    account: Option<Account>,
    device: Option<Device>,
    session: Option<Session>,
    hash: String,
}
#[derive(Default)]
struct Store {
    state: RefCell<State>,
    limit: Cell<usize>,
    touches: Cell<usize>,
}
fn grant(s: &State) -> SessionGrant {
    SessionGrant {
        account: s.account.as_ref().unwrap().clone(),
        session: s.session.as_ref().unwrap().clone(),
    }
}
impl AuthStore for Store {
    async fn has_accounts(&self) -> AuthResult<bool> {
        Ok(self.state.borrow().account.is_some())
    }
    async fn initialize_account_if_empty(&self, c: InitialAccountWrite) -> AuthResult<()> {
        let mut s = self.state.borrow_mut();
        if s.account.is_none() {
            s.account = Some(Account {
                id: 11,
                username: c.username,
                password_hash: c.password_hash,
                deleted_at: None,
            });
        }
        Ok(())
    }
    async fn account_by_username(&self, name: &str) -> AuthResult<Option<Account>> {
        Ok(self
            .state
            .borrow()
            .account
            .as_ref()
            .filter(|a| a.username == name)
            .cloned())
    }
    async fn account_by_id(&self, id: i64) -> AuthResult<Option<Account>> {
        Ok(self
            .state
            .borrow()
            .account
            .as_ref()
            .filter(|a| a.id == id)
            .cloned())
    }
    async fn first_account(&self) -> AuthResult<Option<Account>> {
        Ok(self.state.borrow().account.clone())
    }
    async fn auth_state(&self, id: i64) -> AuthResult<Option<AuthState>> {
        let s = self.state.borrow();
        Ok(match (&s.account, &s.device, &s.session) {
            (Some(a), Some(d), Some(session)) if session.id == id => Some(AuthState {
                account: a.clone(),
                device: d.clone(),
                session: session.clone(),
            }),
            _ => None,
        })
    }
    async fn complete_login(&self, c: LoginWrite) -> AuthResult<SessionGrant> {
        let mut s = self.state.borrow_mut();
        let a = s.account.as_ref().ok_or(AuthError::Unauthorized)?;
        if a.id != c.user_id
            || a.password_hash != c.expected_password_hash
            || a.deleted_at.is_some()
        {
            return Err(AuthError::Unauthorized);
        }
        self.limit.set(c.max_devices);
        s.device = Some(Device {
            id: 22,
            user_id: c.user_id,
            device_id: c.device.device_id,
            name: Some(c.device.device_name),
            platform: c.device.platform,
            device_type: c.device.device_type,
            manufacturer: Some(c.device.manufacturer),
            model: Some(c.device.model),
            os_version: Some(c.device.os_version),
            app_version: Some(c.device.app_version),
            app_build: Some(c.device.app_build),
            locale: Some(c.device.locale),
            time_zone: Some(c.device.time_zone),
            ip_address: Some(c.meta.ip),
            user_agent: Some(c.meta.user_agent),
            last_seen_at: Some(c.now),
            created_at: c.now,
            updated_at: c.now,
            deleted_at: None,
        });
        s.session = Some(Session {
            id: 33,
            device_id: 22,
            expires_at: c.expires_at,
            logged_in_at: c.now,
            revoked_at: None,
            deleted_at: None,
        });
        s.hash = c.refresh_token_hash;
        Ok(grant(&s))
    }
    async fn rotate_refresh_token(&self, c: RefreshWrite) -> AuthResult<SessionGrant> {
        let mut s = self.state.borrow_mut();
        if s.hash != c.previous_hash
            || !s.session.as_ref().is_some_and(|v| v.active(c.now))
            || s.account.as_ref().unwrap().deleted_at.is_some()
            || s.device.as_ref().unwrap().deleted_at.is_some()
        {
            return Err(AuthError::Unauthorized);
        }
        s.hash = c.new_hash;
        let session = s.session.as_mut().unwrap();
        session.expires_at = c.expires_at;
        let device = s.device.as_mut().unwrap();
        device.ip_address = Some(c.meta.ip);
        device.user_agent = Some(c.meta.user_agent);
        device.last_seen_at = Some(c.now);
        Ok(grant(&s))
    }
    async fn revoke_refresh_token(&self, hash: &str, now: DateTime<Utc>) -> AuthResult<bool> {
        let mut s = self.state.borrow_mut();
        let valid = s.hash == hash && s.session.as_ref().is_some_and(|v| v.active(now));
        if valid {
            s.session.as_mut().unwrap().revoked_at = Some(now);
        }
        Ok(valid)
    }
    async fn update_account(&self, c: AccountWrite) -> AuthResult<Account> {
        let mut s = self.state.borrow_mut();
        let a = s.account.as_mut().unwrap();
        if a.id != c.user_id || a.password_hash != c.expected_password_hash {
            return Err(AuthError::Unauthorized);
        }
        a.username = c.username;
        if let Some(hash) = c.new_password_hash {
            a.password_hash = hash;
            if let Some(session) = &mut s.session {
                session.revoked_at = Some(c.now);
            }
        }
        Ok(s.account.as_ref().unwrap().clone())
    }
    async fn touch_device(
        &self,
        _id: i64,
        before: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
        let mut s = self.state.borrow_mut();
        let d = s.device.as_mut().unwrap();
        if d.last_seen_at.is_none_or(|seen| seen < before) {
            d.last_seen_at = Some(now);
            self.touches.set(self.touches.get() + 1);
        }
        Ok(())
    }
    async fn devices(&self, uid: i64) -> AuthResult<Vec<DeviceSession>> {
        let s = self.state.borrow();
        Ok(s.device
            .as_ref()
            .filter(|d| d.user_id == uid)
            .map(|d| {
                vec![DeviceSession {
                    device: d.clone(),
                    session: s.session.clone(),
                }]
            })
            .unwrap_or_default())
    }
    async fn revoke_device_session(
        &self,
        uid: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> AuthResult<()> {
        let mut s = self.state.borrow_mut();
        if !s
            .device
            .as_ref()
            .is_some_and(|d| d.user_id == uid && d.device_id == identifier)
        {
            return Err(AuthError::NotFound("device not found"));
        }
        if let Some(session) = &mut s.session {
            session.revoked_at = Some(now);
        }
        Ok(())
    }
    async fn delete_device(&self, uid: i64, identifier: &str) -> AuthResult<()> {
        let mut s = self.state.borrow_mut();
        if !s
            .device
            .as_ref()
            .is_some_and(|d| d.user_id == uid && d.device_id == identifier)
        {
            return Err(AuthError::NotFound("device not found"));
        }
        s.device = None;
        s.session = None;
        Ok(())
    }
}

#[tokio::test]
async fn shared_flow_uses_one_clock_token_policy_metadata_and_revocation_policy() {
    let service = AuthService {
        store: Store::default(),
        runtime: Runtime::new(),
        config: config(),
    };
    let result = service
        .login(
            request("admin", "password123"),
            RequestMeta::new("ip-before".into(), "login-agent".into()),
        )
        .await
        .unwrap();
    assert!(result.user.requires_account_setup);
    assert_eq!(service.store.limit.get(), MAX_DEVICES);
    assert_eq!(
        result.access_token_expires_at.timestamp(),
        1000 + ACCESS_TTL
    );
    assert_eq!(
        result.refresh_token_expires_at.timestamp(),
        1000 + REFRESH_DAYS * 86_400
    );
    assert_eq!(
        service.store.state.borrow().hash,
        crate::security::hash_token(&result.refresh_token)
    );
    let context = service.authenticate(&result.access_token).await.unwrap();
    assert_eq!(context.user_id, 11);
    assert_eq!(context.device_id, 22);
    service.runtime.seconds.set(1300);
    service.authenticate(&result.access_token).await.unwrap();
    assert_eq!(service.store.touches.get(), 0);
    service.runtime.seconds.set(1301);
    service.authenticate(&result.access_token).await.unwrap();
    assert_eq!(service.store.touches.get(), 1);
    let rotated = service
        .refresh(
            RefreshRequest {
                refresh_token: result.refresh_token.clone(),
            },
            RequestMeta::new("ip-after".into(), "refresh-agent".into()),
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .refresh(
                RefreshRequest {
                    refresh_token: result.refresh_token
                },
                RequestMeta::default()
            )
            .await,
        Err(AuthError::Unauthorized)
    ));
    let devices = service.devices(11).await.unwrap();
    assert!(devices[0].logged_in);
    assert_eq!(devices[0].ip_address, "ip-after");
    assert_eq!(devices[0].user_agent, "refresh-agent");
    service
        .update_account(
            11,
            AccountUpdateRequest {
                current_password: "password123".into(),
                username: String::new(),
                new_password: "newpassword123".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        service.authenticate(&rotated.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    assert!(matches!(
        service
            .login(request("admin", "password123"), RequestMeta::default())
            .await,
        Err(AuthError::Unauthorized)
    ));
    let login = service
        .login(request("admin", "newpassword123"), RequestMeta::default())
        .await
        .unwrap();
    assert!(!login.user.requires_account_setup);
    service.kick_device(11, "phone").await.unwrap();
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    service
        .reset_account(Some("renamed".into()), "resetpassword123")
        .await
        .unwrap();
    let login = service
        .login(
            request("renamed", "resetpassword123"),
            RequestMeta::default(),
        )
        .await
        .unwrap();
    service.delete_device(11, "phone").await.unwrap();
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
}

#[tokio::test]
async fn authenticate_rejects_deleted_accounts_devices_and_ownership_mismatch() {
    let service = AuthService {
        store: Store::default(),
        runtime: Runtime::new(),
        config: config(),
    };
    let login = service
        .login(request("admin", "password123"), RequestMeta::default())
        .await
        .unwrap();
    service
        .store
        .state
        .borrow_mut()
        .account
        .as_mut()
        .unwrap()
        .deleted_at = Some(service.runtime.now());
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    service
        .store
        .state
        .borrow_mut()
        .account
        .as_mut()
        .unwrap()
        .deleted_at = None;
    service
        .store
        .state
        .borrow_mut()
        .device
        .as_mut()
        .unwrap()
        .deleted_at = Some(service.runtime.now());
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    service
        .store
        .state
        .borrow_mut()
        .device
        .as_mut()
        .unwrap()
        .deleted_at = None;
    service
        .store
        .state
        .borrow_mut()
        .device
        .as_mut()
        .unwrap()
        .user_id = 99;
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    service
        .store
        .state
        .borrow_mut()
        .device
        .as_mut()
        .unwrap()
        .user_id = 11;
    service
        .store
        .state
        .borrow_mut()
        .session
        .as_mut()
        .unwrap()
        .expires_at = service.runtime.now();
    assert!(matches!(
        service.authenticate(&login.access_token).await,
        Err(AuthError::Unauthorized)
    ));
    assert_eq!(service.store.touches.get(), 0);
}

#[test]
fn jwt_checks_signature_algorithm_issuer_ids_and_exact_expiry_without_a_system_clock() {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    let config = config();
    let now = DateTime::from_timestamp(1000, 0).unwrap();
    let (signed, expires) = token::issue(&config, 11, 33, now).unwrap();
    assert!(token::decode(&config, &signed, expires - Duration::seconds(1)).is_ok());
    assert!(matches!(
        token::decode(&config, &signed, expires),
        Err(AuthError::Unauthorized)
    ));
    let base = token::decode(&config, &signed, now).unwrap();
    for mutate in [0, 1, 2, 3] {
        let mut claims = base.clone();
        let mut algorithm = Algorithm::HS256;
        match mutate {
            0 => claims.user_id = 0,
            1 => claims.session_id = 0,
            2 => claims.iss = "other-issuer".into(),
            _ => algorithm = Algorithm::HS384,
        };
        let signed = jsonwebtoken::encode(
            &Header::new(algorithm),
            &claims,
            &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
        )
        .unwrap();
        assert!(matches!(
            token::decode(&config, &signed, now),
            Err(AuthError::Unauthorized)
        ));
    }
    let forged = jsonwebtoken::encode(
        &Header::default(),
        &base,
        &EncodingKey::from_secret(b"wrong-secret"),
    )
    .unwrap();
    assert!(matches!(
        token::decode(&config, &forged, now),
        Err(AuthError::Unauthorized)
    ));
    assert!(matches!(
        token::decode(&config, "malformed", now),
        Err(AuthError::Unauthorized)
    ));
}

#[test]
fn bcrypt_policy_is_shared_and_bad_stored_hash_is_not_a_bad_password() {
    let hash = password::hash("password123", [7; 16]).unwrap();
    assert!(hash.starts_with("$2b$10$"));
    assert!(password::verify("password123", &hash).unwrap());
    assert!(!password::verify("incorrect", &hash).unwrap());
    assert!(matches!(
        password::verify("password123", "damaged"),
        Err(AuthError::Internal(_))
    ));
    assert!(matches!(
        password::hash("short1", [7; 16]),
        Err(AuthError::InvalidInput(_))
    ));
}

#[tokio::test]
async fn initialization_never_overwrites_existing_or_soft_deleted_accounts() {
    let mut service = AuthService {
        store: Store::default(),
        runtime: Runtime::new(),
        config: config(),
    };
    service.ensure_initial_account().await.unwrap();
    service
        .store
        .state
        .borrow_mut()
        .account
        .as_mut()
        .unwrap()
        .deleted_at = Some(service.runtime.now());
    service.config.initial_username = "must-not-replace".into();
    service.config.initial_password = String::new();
    service.ensure_initial_account().await.unwrap();
    let state = service.store.state.borrow();
    let account = state.account.as_ref().unwrap();
    assert_eq!(account.username, "admin");
    assert!(account.deleted_at.is_some());
}

#[tokio::test]
async fn invalid_signing_configuration_fails_before_account_or_session_writes() {
    let mut config = config();
    config.jwt_secret.clear();
    let service = AuthService {
        store: Store::default(),
        runtime: Runtime::new(),
        config,
    };
    assert!(matches!(
        service
            .login(request("admin", "password123"), RequestMeta::default())
            .await,
        Err(AuthError::Internal(_))
    ));
    assert!(service.store.state.borrow().account.is_none());
}
