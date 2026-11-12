use super::*;
use chrono::{DateTime, Duration, Utc};

pub struct AuthService<S, R> {
    pub store: S,
    pub runtime: R,
    pub config: AuthConfig,
}

impl<S: AuthStore, R: AuthRuntime> AuthService<S, R> {
    fn random_token(&self) -> AuthResult<String> {
        let mut bytes = [0; 32];
        self.runtime.random_bytes(&mut bytes)?;
        Ok(hex::encode(bytes))
    }
    async fn hash_password(&self, password: &str) -> AuthResult<String> {
        crate::security::validate_password(password)
            .map_err(|message| AuthError::InvalidInput(message.into()))?;
        let mut salt = [0; 16];
        self.runtime.random_bytes(&mut salt)?;
        self.runtime.hash_password(password, salt).await
    }
    async fn verify_password(&self, password: &str, hash: &str) -> AuthResult<()> {
        if self.runtime.verify_password(password, hash).await? {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }
    fn active_account(account: Option<Account>) -> AuthResult<Account> {
        account
            .filter(|a| a.deleted_at.is_none())
            .ok_or(AuthError::Unauthorized)
    }
    pub async fn is_initialized(&self) -> AuthResult<bool> {
        self.store.has_accounts().await
    }
    pub async fn initialize_account(&self, request: InitializeAccount) -> AuthResult<()> {
        token::validate_config(&self.config)?;
        if self.store.has_accounts().await? {
            return Err(AuthError::Conflict("account already initialized"));
        }
        request.validate().map_err(AuthError::InvalidInput)?;
        let hash = self.hash_password(&request.password).await?;
        self.store
            .initialize_account_if_empty(InitialAccountWrite {
                username: request.username,
                password_hash: hash,
                now: self.runtime.now(),
            })
            .await
    }
    fn tokens(
        &self,
        grant: SessionGrant,
        refresh: String,
        now: DateTime<Utc>,
    ) -> AuthResult<SessionTokens> {
        let (access, expiry) = token::issue(&self.config, grant.account.id, grant.session.id, now)?;
        Ok(SessionTokens {
            access_token: access,
            access_token_expires_at: expiry,
            refresh_token: refresh,
            refresh_token_expires_at: grant.session.expires_at,
            user: AccountInfo {
                id: grant.account.id,
                username: grant.account.username,
            },
        })
    }
    pub async fn login(&self, request: Login, meta: RequestMeta) -> AuthResult<SessionTokens> {
        token::validate_config(&self.config)?;
        request.validate().map_err(AuthError::InvalidInput)?;
        let account =
            Self::active_account(self.store.account_by_username(&request.username).await?)?;
        self.verify_password(&request.password, &account.password_hash)
            .await?;
        let now = self.runtime.now();
        let refresh = self.random_token()?;
        let grant = self
            .store
            .complete_login(LoginWrite {
                user_id: account.id,
                expected_password_hash: account.password_hash,
                device: request.device,
                meta,
                refresh_token_hash: crate::security::hash_token(&refresh),
                expires_at: now + Duration::days(REFRESH_DAYS),
                now,
                max_devices: MAX_DEVICES,
            })
            .await?;
        self.tokens(grant, refresh, now)
    }
    pub async fn refresh(
        &self,
        refresh_token: &str,
        meta: RequestMeta,
    ) -> AuthResult<SessionTokens> {
        token::validate_config(&self.config)?;
        check_length(refresh_token, "refreshToken", usize::MAX, true)
            .map_err(AuthError::InvalidInput)?;
        let now = self.runtime.now();
        let refresh = self.random_token()?;
        let grant = self
            .store
            .rotate_refresh_token(RefreshWrite {
                previous_hash: crate::security::hash_token(refresh_token),
                new_hash: crate::security::hash_token(&refresh),
                expires_at: now + Duration::days(REFRESH_DAYS),
                now,
                meta,
            })
            .await?;
        self.tokens(grant, refresh, now)
    }
    pub async fn logout(&self, refresh_token: &str) -> AuthResult<()> {
        check_length(refresh_token, "refreshToken", usize::MAX, true)
            .map_err(AuthError::InvalidInput)?;
        if self
            .store
            .revoke_refresh_token(
                &crate::security::hash_token(refresh_token),
                self.runtime.now(),
            )
            .await?
        {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }
    pub async fn authenticate(&self, value: &str) -> AuthResult<AuthContext> {
        let now = self.runtime.now();
        let claims = token::decode(&self.config, value, now)?;
        let state = self
            .store
            .auth_state(claims.session_id)
            .await?
            .ok_or(AuthError::Unauthorized)?;
        if state.account.deleted_at.is_some()
            || state.device.deleted_at.is_some()
            || !state.session.active(now)
            || state.session.id != claims.session_id
            || state.account.id != claims.user_id
            || state.device.user_id != state.account.id
            || state.session.device_id != state.device.id
        {
            return Err(AuthError::Unauthorized);
        }
        let expires_at = DateTime::from_timestamp(claims.exp, 0).ok_or(AuthError::Unauthorized)?;
        let before = now - Duration::seconds(LAST_SEEN_SECONDS);
        if state.device.last_seen_at.is_none_or(|seen| seen < before) {
            self.store
                .touch_device(state.device.id, before, now)
                .await?;
        }
        Ok(AuthContext {
            user_id: claims.user_id,
            session_id: claims.session_id,
            device_id: state.device.id,
            expires_at,
        })
    }
    pub async fn account(&self, user_id: i64) -> AuthResult<AccountInfo> {
        let account = Self::active_account(self.store.account_by_id(user_id).await?)?;
        Ok(AccountInfo {
            id: account.id,
            username: account.username,
        })
    }
    pub async fn update_account(
        &self,
        user_id: i64,
        request: UpdateAccount,
    ) -> AuthResult<AccountInfo> {
        request.validate().map_err(AuthError::InvalidInput)?;
        let account = Self::active_account(self.store.account_by_id(user_id).await?)?;
        self.verify_password(&request.current_password, &account.password_hash)
            .await?;
        let hash = if request.new_password.is_empty() {
            None
        } else {
            Some(self.hash_password(&request.new_password).await?)
        };
        let account = self
            .store
            .update_account(AccountWrite {
                user_id,
                expected_password_hash: account.password_hash,
                username: if request.username.is_empty() {
                    account.username
                } else {
                    request.username
                },
                new_password_hash: hash,
                now: self.runtime.now(),
            })
            .await?;
        Ok(AccountInfo {
            id: account.id,
            username: account.username,
        })
    }
    /// Privileged local operation; no public password-reset HTTP endpoint.
    pub async fn reset_account(&self, username: Option<String>, password: &str) -> AuthResult<()> {
        if let Some(name) = &username {
            check_length(name, "username", 64, true).map_err(AuthError::InvalidInput)?;
        }
        let hash = self.hash_password(password).await?;
        let account = self
            .store
            .first_account()
            .await?
            .filter(|a| a.deleted_at.is_none())
            .ok_or(AuthError::NotFound("account not found"))?;
        self.store
            .update_account(AccountWrite {
                user_id: account.id,
                expected_password_hash: account.password_hash,
                username: username.unwrap_or(account.username),
                new_password_hash: Some(hash),
                now: self.runtime.now(),
            })
            .await?;
        Ok(())
    }
    pub async fn devices(&self, user_id: i64) -> AuthResult<Vec<DeviceInfo>> {
        let now = self.runtime.now();
        let mut items = Vec::new();
        for row in self.store.devices(user_id).await? {
            let d = row.device;
            if d.deleted_at.is_some() {
                continue;
            }
            let session = row.session.filter(|s| s.deleted_at.is_none());
            items.push(DeviceInfo {
                logged_in: session.as_ref().is_some_and(|s| s.active(now)),
                device: d,
                session,
            });
        }
        items.sort_by(|a, b| {
            b.logged_in
                .cmp(&a.logged_in)
                .then(b.device.updated_at.cmp(&a.device.updated_at))
                .then(a.device.device_id.cmp(&b.device.device_id))
        });
        Ok(items)
    }
    pub async fn kick_device(&self, user_id: i64, identifier: &str) -> AuthResult<()> {
        check_length(identifier, "deviceId", 128, true).map_err(AuthError::InvalidInput)?;
        self.store
            .revoke_device_session(user_id, identifier, self.runtime.now())
            .await
    }
    pub async fn delete_device(&self, user_id: i64, identifier: &str) -> AuthResult<()> {
        check_length(identifier, "deviceId", 128, true).map_err(AuthError::InvalidInput)?;
        self.store.delete_device(user_id, identifier).await
    }
}
