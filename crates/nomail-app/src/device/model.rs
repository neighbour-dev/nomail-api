use crate::auth::Session;
use chrono::{DateTime, Utc};
use garde::Validate;
use serde::Deserialize;

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
    super::validate_id(value).map_err(garde::Error::new)
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

#[cfg(test)]
mod tests {
    use super::DeviceInput;
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
}
