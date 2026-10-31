use crate::error::Result;

pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(bytes)
}
pub fn message_id(now_ms: u64) -> Result<uuid::Uuid> {
    let mut bytes = random::<16>()?;
    bytes[..6].copy_from_slice(&now_ms.to_be_bytes()[2..]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(uuid::Uuid::from_bytes(bytes))
}
