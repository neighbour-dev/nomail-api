use base64::{Engine, engine::general_purpose::STANDARD};
use rand::{RngCore, rngs::OsRng};
fn main() {
    let mut key = [0; 32];
    OsRng.fill_bytes(&mut key);
    println!("SECRET_ENCRYPTION_KEY={}", STANDARD.encode(key));
}
