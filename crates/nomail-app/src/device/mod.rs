//! Authenticated device management, independent of login and HTTP dispatch.
mod error;
mod model;

pub use error::{DeviceError, DeviceResult};
pub use model::{Device, DeviceInfo, DeviceInput, DeviceSession};

use crate::{auth::AuthContext, push::Channel};
use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct PushPreferences {
    pub enabled: bool,
    pub low: bool,
    pub normal: bool,
    pub high: bool,
    pub urgent: bool,
}

impl Default for PushPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            low: true,
            normal: true,
            high: true,
            urgent: true,
        }
    }
}

#[derive(Validate)]
pub struct PushRegistration {
    #[garde(skip)]
    pub channel: Channel,
    #[garde(
        if(cond = self.channel == Channel::Apns, length(bytes, min = 2, max = 512), custom(validate_apns)),
        if(cond = self.channel == Channel::Fcm, length(bytes, min = 1, max = 4096), custom(validate_fcm))
    )]
    pub token: String,
}

impl PushRegistration {
    fn new(channel: Channel, token: String) -> DeviceResult<Self> {
        let mut input = Self { channel, token };
        input.validate().map_err(|_| {
            DeviceError::InvalidInput(
                match channel {
                    Channel::Apns => "invalid APNs device token",
                    Channel::Fcm => "invalid FCM registration token",
                }
                .into(),
            )
        })?;
        if channel == Channel::Apns {
            input.token.make_ascii_lowercase();
        }
        Ok(input)
    }
}

fn validate_apns(token: &str, _: &()) -> garde::Result {
    if token.len().is_multiple_of(2) && token.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(garde::Error::new(
            "must contain complete hexadecimal byte pairs",
        ))
    }
}

fn validate_fcm(token: &str, _: &()) -> garde::Result {
    if token.bytes().all(|b| b.is_ascii_graphic()) {
        Ok(())
    } else {
        Err(garde::Error::new(
            "must contain only visible ASCII characters",
        ))
    }
}

#[allow(async_fn_in_trait)]
pub trait DeviceStore {
    async fn push_preferences(&self, device_id: i64) -> DeviceResult<PushPreferences>;
    async fn set_push_preferences(
        &self,
        actor: &AuthContext,
        preferences: PushPreferences,
        now: DateTime<Utc>,
    ) -> DeviceResult<()>;
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
    pub async fn push_preferences(&self, actor: &AuthContext) -> DeviceResult<PushPreferences> {
        self.store.push_preferences(actor.device_id).await
    }

    pub async fn set_push_preferences(
        &self,
        actor: &AuthContext,
        preferences: PushPreferences,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        self.store
            .set_push_preferences(actor, preferences, now)
            .await
    }

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
        identifier: DeviceId<'_>,
        now: DateTime<Utc>,
    ) -> DeviceResult<()> {
        self.store
            .revoke_device_session(user_id, identifier.as_str(), now)
            .await
    }

    pub async fn delete(&self, user_id: i64, identifier: DeviceId<'_>) -> DeviceResult<()> {
        self.store.delete_device(user_id, identifier.as_str()).await
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

/// A validated device identifier borrowed from its input.
#[derive(Validate)]
#[garde(transparent)]
pub struct DeviceId<'a>(#[garde(length(chars, min = 1, max = 128))] &'a str);

impl<'a> DeviceId<'a> {
    pub fn new(identifier: &'a str) -> DeviceResult<Self> {
        let id = Self(identifier);
        id.validate()
            .map_err(|_| DeviceError::InvalidInput("invalid deviceId".into()))?;
        Ok(id)
    }

    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Channel, DeviceError, PushRegistration};

    #[test]
    fn push_registration_uses_channel_specific_token_rules() {
        let apns = PushRegistration::new(Channel::Apns, "AF01".into()).unwrap();
        assert_eq!(apns.token, "af01");
        let fcm = PushRegistration::new(Channel::Fcm, "AbC:_-".into()).unwrap();
        assert_eq!(fcm.token, "AbC:_-");

        for (channel, token, expected) in [
            (Channel::Apns, "a".repeat(513), "invalid APNs device token"),
            (Channel::Apns, "abc".into(), "invalid APNs device token"),
            (Channel::Apns, "gg".into(), "invalid APNs device token"),
            (
                Channel::Fcm,
                String::new(),
                "invalid FCM registration token",
            ),
            (
                Channel::Fcm,
                "x".repeat(4097),
                "invalid FCM registration token",
            ),
            (Channel::Fcm, "a b".into(), "invalid FCM registration token"),
        ] {
            assert!(matches!(
                PushRegistration::new(channel, token),
                Err(DeviceError::InvalidInput(message)) if message == expected
            ));
        }
    }
}
