fn random<const N: usize>() -> Result<[u8; N], getrandom::Error> {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes)?;
    Ok(bytes)
}
pub fn message_id(now_ms: u64) -> Result<uuid::Uuid, getrandom::Error> {
    let mut bytes = random::<16>()?;
    bytes[..6].copy_from_slice(&now_ms.to_be_bytes()[2..]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(uuid::Uuid::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_identity_keeps_uuid_v7_timestamp_version_and_variant() {
        let now = 1_800_000_000_123_u64;
        let id = message_id(now).unwrap();
        assert_eq!(&id.as_bytes()[..6], &now.to_be_bytes()[2..]);
        assert_eq!(id.get_version_num(), 7);
        assert_eq!(id.get_variant(), uuid::Variant::RFC4122);
        assert_ne!(id, message_id(now).unwrap());
    }
}
