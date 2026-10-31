//! JWT payload, not an HTTP request/response DTO.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: i64,
    pub session_id: i64,
    pub sub: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
}
