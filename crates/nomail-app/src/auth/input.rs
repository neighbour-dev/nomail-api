use crate::device::DeviceInput;
use garde::Validate;

const MAX_USERNAME_LENGTH: usize = 64;

#[derive(Validate)]
#[garde(transparent)]
pub(super) struct RefreshToken<'a>(#[garde(length(bytes, min = 1))] pub &'a str);

/// First-account credentials. Deliberately not Debug/Serialize.
#[derive(Validate)]
pub struct InitializeAccount {
    #[garde(length(chars, min = 1, max = MAX_USERNAME_LENGTH))]
    pub username: String,
    #[garde(custom(validate_password))]
    pub password: String,
}

/// Application credentials and device registration, independent of wire fields.
/// Deliberately not Debug/Serialize: contains a plaintext password.
#[derive(Validate)]
pub struct Login {
    #[garde(length(chars, min = 1, max = MAX_USERNAME_LENGTH))]
    pub username: String,
    #[garde(length(bytes, min = 1))]
    pub password: String,
    #[garde(dive)]
    pub device: DeviceInput,
}

/// Empty optional replacements retain the existing account, as before.
#[derive(Validate)]
pub struct UpdateAccount {
    #[garde(length(bytes, min = 1))]
    pub current_password: String,
    #[garde(length(chars, max = MAX_USERNAME_LENGTH))]
    pub username: String,
    #[garde(custom(|password, _| validate_replacement(password, &self.username)))]
    pub new_password: String,
}

#[derive(Validate)]
pub(super) struct ResetAccount<'a> {
    #[garde(length(chars, min = 1, max = MAX_USERNAME_LENGTH))]
    pub username: Option<&'a str>,
    #[garde(custom(validate_password))]
    pub password: &'a str,
}

fn validate_password(password: &str, _: &()) -> garde::Result {
    crate::security::validate_password(password).map_err(garde::Error::new)
}

fn validate_replacement(password: &str, username: &str) -> garde::Result {
    if !password.is_empty() {
        return validate_password(password, &());
    }
    if username.is_empty() {
        return Err(garde::Error::new("username or newPassword is required"));
    }
    Ok(())
}
