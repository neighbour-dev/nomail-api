mod error;
mod input;
mod service;
pub use error::{ConfigError, ConfigResult};
pub use input::{CreateConfig, UpdateConfig};
pub use service::*;

#[cfg(test)]
mod tests;

pub struct ConfigView {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub provider: super::Provider,
    pub source: String,
    pub enabled: bool,
    pub description: String,
}
impl ConfigView {
    pub fn new(
        id: i64,
        name: String,
        key: String,
        provider: super::Provider,
        source: String,
        enabled: bool,
        description: String,
    ) -> Self {
        Self {
            id,
            name,
            key,
            provider,
            source,
            enabled,
            description,
        }
    }
}
