use super::DeviceInput;

pub fn check_length(value: &str, name: &str, max: usize, required: bool) -> Result<(), String> {
    if (required && value.is_empty()) || value.chars().count() > max {
        Err(format!("invalid {name}"))
    } else {
        Ok(())
    }
}

/// First-account credentials. Deliberately not Debug/Serialize.
pub struct InitializeAccount {
    pub username: String,
    pub password: String,
}
impl InitializeAccount {
    pub(super) fn validate(&self) -> Result<(), String> {
        check_length(&self.username, "username", 64, true)?;
        crate::security::validate_password(&self.password).map_err(String::from)
    }
}

/// Application credentials and device registration, independent of wire fields.
/// Deliberately not Debug/Serialize: contains a plaintext password.
pub struct Login {
    pub username: String,
    pub password: String,
    pub device: DeviceInput,
}
impl Login {
    pub(super) fn validate(&self) -> Result<(), String> {
        check_length(&self.username, "username", 64, true)?;
        check_length(&self.password, "password", usize::MAX, true)?;
        let device = &self.device;
        check_length(&device.device_id, "deviceId", 128, true)?;
        if ![
            "ios", "android", "macos", "windows", "linux", "web", "unknown",
        ]
        .contains(&device.platform.as_str())
        {
            return Err("unsupported platform".into());
        }
        if !["phone", "tablet", "desktop", "web", "unknown"].contains(&device.device_type.as_str())
        {
            return Err("unsupported deviceType".into());
        }
        for (name, value, max) in [
            ("deviceName", &device.device_name, 128),
            ("manufacturer", &device.manufacturer, 128),
            ("model", &device.model, 128),
            ("osVersion", &device.os_version, 64),
            ("appVersion", &device.app_version, 64),
            ("appBuild", &device.app_build, 64),
            ("locale", &device.locale, 32),
            ("timeZone", &device.time_zone, 64),
        ] {
            check_length(value, name, max, false)?;
        }
        Ok(())
    }
}
/// Empty optional replacements retain the existing account, as before.
pub struct UpdateAccount {
    pub current_password: String,
    pub username: String,
    pub new_password: String,
}
impl UpdateAccount {
    pub(super) fn validate(&self) -> Result<(), String> {
        check_length(&self.current_password, "currentPassword", usize::MAX, true)?;
        check_length(&self.username, "username", 64, false)?;
        if self.username.is_empty() && self.new_password.is_empty() {
            return Err("username or newPassword is required".into());
        }
        if !self.new_password.is_empty() {
            crate::security::validate_password(&self.new_password).map_err(String::from)?;
        }
        Ok(())
    }
}
