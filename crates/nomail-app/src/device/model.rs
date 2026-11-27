use super::{DeviceError, DeviceResult};
use crate::{auth::Session, push::Channel};
use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct Device {
    pub id: i64,
    pub user_id: i64,
    pub device_id: String,
    pub name: Option<String>,
    pub platform: String,
    pub device_type: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub os_version: Option<String>,
    pub app_version: Option<String>,
    pub app_build: Option<String>,
    pub locale: Option<String>,
    pub time_zone: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub struct DeviceSession {
    pub device: Device,
    pub session: Option<Session>,
}

pub struct DeviceInfo {
    pub device: Device,
    pub session: Option<Session>,
    pub logged_in: bool,
}

/// Device metadata attached to a login; never contains credentials.
#[derive(Default, Validate)]
pub struct DeviceInput {
    #[garde(custom(validate_id))]
    pub device_id: String,
    #[garde(custom(validate_platform))]
    pub platform: String,
    #[garde(custom(validate_device_type))]
    pub device_type: String,
    #[garde(length(chars, max = 128))]
    pub device_name: String,
    #[garde(length(chars, max = 128))]
    pub manufacturer: String,
    #[garde(length(chars, max = 128))]
    pub model: String,
    #[garde(length(chars, max = 64))]
    pub os_version: String,
    #[garde(length(chars, max = 64))]
    pub app_version: String,
    #[garde(length(chars, max = 64))]
    pub app_build: String,
    #[garde(length(chars, max = 32))]
    pub locale: String,
    #[garde(length(chars, max = 64))]
    pub time_zone: String,
}

fn validate_id(value: &str, _: &()) -> garde::Result {
    DeviceId::new(value)
        .map(|_| ())
        .map_err(|error| garde::Error::new(error.to_string()))
}

fn validate_platform(value: &str, _: &()) -> garde::Result {
    match value {
        "ios" | "android" | "macos" | "windows" | "linux" | "web" | "unknown" => Ok(()),
        _ => Err(garde::Error::new("unsupported platform")),
    }
}

fn validate_device_type(value: &str, _: &()) -> garde::Result {
    match value {
        "phone" | "tablet" | "desktop" | "web" | "unknown" => Ok(()),
        _ => Err(garde::Error::new("unsupported deviceType")),
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
    pub(super) fn new(channel: Channel, token: String) -> DeviceResult<Self> {
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

#[cfg(test)]
mod tests {
    use super::{Channel, DeviceError, DeviceInput, PushRegistration};
    use crate::auth::Login;
    use crate::error::validate_input;

    #[test]
    fn login_validates_nested_fields_in_one_pass() {
        let mut login = Login {
            username: "user".into(),
            password: "existing-password".into(),
            device: DeviceInput {
                device_id: "device".into(),
                platform: "ios".into(),
                device_type: "phone".into(),
                device_name: "中".repeat(128),
                ..DeviceInput::default()
            },
        };
        assert!(validate_input(&login).is_ok());
        login.device.device_name.push('中');
        login.device.platform = "invalid".into();
        login.username.clear();
        let report = validate_input(&login).unwrap_err();
        for field in ["username", "device.device_name", "device.platform"] {
            assert!(report.contains(field), "{report}");
        }
    }

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
