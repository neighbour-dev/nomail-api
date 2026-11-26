//! Authenticated device management, independent of login and HTTP dispatch.
mod error;
mod model;

pub use error::{DeviceError, DeviceResult};
pub use model::{Device, DeviceInfo, DeviceInput, DeviceSession};

use crate::{auth::AuthContext, push::Channel};
use chrono::{DateTime, Utc};

pub struct PushRegistration {
    pub channel: Channel,
    pub token: String,
}

impl PushRegistration {
    fn new(channel: Channel, token: String) -> DeviceResult<Self> {
        let token = match channel {
            Channel::Apns => {
                if !(2..=512).contains(&token.len())
                    || !token.len().is_multiple_of(2)
                    || !token.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(DeviceError::InvalidInput(
                        "invalid APNs device token".into(),
                    ));
                }
                token.to_ascii_lowercase()
            }
            Channel::Fcm => {
                if !(1..=4096).contains(&token.len())
                    || !token.bytes().all(|b| b.is_ascii_graphic())
                {
                    return Err(DeviceError::InvalidInput(
                        "invalid FCM registration token".into(),
                    ));
                }
                token
            }
        };
        Ok(Self { channel, token })
    }
}

#[allow(async_fn_in_trait)]
pub trait DeviceStore {
    async fn devices(&self, user_id: i64) -> DeviceResult<Vec<DeviceSession>>;
    async fn revoke_device_session(
        &self,
        user_id: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> DeviceResult<()>;
    async fn delete_device(&self, user_id: i64, identifier: &str) -> DeviceResult<()>;
    /// Recheck the active session and update only its target in one operation.
    /// None unregisters it; reposting the same target preserves its version.
    async fn set_target(
        &self,
        actor: &AuthContext,
        target: Option<&PushRegistration>,
        now: DateTime<Utc>,
    ) -> DeviceResult<()>;
}

pub struct DeviceService<S> {
    pub store: S,
}

impl<S: DeviceStore> DeviceService<S> {
    pub async fn list(&self, user_id: i64, now: DateTime<Utc>) -> DeviceResult<Vec<DeviceInfo>> {
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

    pub async fn revoke_session(
        &self,
        user_id: i64,
        identifier: &str,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        validate_id(identifier).map_err(DeviceError::InvalidInput)?;
        self.store
            .revoke_device_session(user_id, identifier, now)
            .await
    }

    pub async fn delete(&self, user_id: i64, identifier: &str) -> DeviceResult<()> {
        validate_id(identifier).map_err(DeviceError::InvalidInput)?;
        self.store.delete_device(user_id, identifier).await
    }

    pub async fn register_push(
        &self,
        actor: &AuthContext,
        channel: Channel,
        token: String,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        let registration = PushRegistration::new(channel, token)?;
        self.store.set_target(actor, Some(&registration), now).await
    }

    pub async fn remove_push(&self, actor: &AuthContext, now: DateTime<Utc>) -> DeviceResult<()> {
        self.store.set_target(actor, None, now).await
    }
}

pub(crate) fn validate_id(identifier: &str) -> Result<(), String> {
    if identifier.is_empty() || identifier.chars().count() > 128 {
        return Err("invalid deviceId".into());
    }
    Ok(())
}
