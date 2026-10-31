use super::security::*;
use base64::{Engine, engine::general_purpose::STANDARD};

#[test]
fn encrypted_credentials_are_portable_and_authenticated() {
    let key = [7; 32];
    let value = encrypt("secret 中文", &key, &[1; 12]).unwrap();
    assert_eq!(&STANDARD.decode(&value).unwrap()[..12], &[1; 12]);
    assert_eq!(decrypt(&value, &key).unwrap(), "secret 中文");
    assert!(decrypt(&value, &[8; 32]).is_err());
    let mut tampered = STANDARD.decode(&value).unwrap();
    tampered[15] ^= 1;
    assert!(decrypt(&STANDARD.encode(tampered), &key).is_err());
    assert!(decrypt("AA==", &key).is_err());
    assert!(decode_key(&STANDARD.encode(key)).is_ok());
    assert!(decode_key(&STANDARD.encode([7; 31])).is_err());
    assert_eq!(
        hash_token("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn password_length_is_checked_in_bytes_for_bcrypt() {
    for value in ["short1", "password", "12345678"] {
        assert!(validate_password(value).is_err());
    }
    assert!(validate_password("password123").is_ok());
    assert!(validate_password(&format!("A1{}", "中".repeat(24))).is_err());
}
