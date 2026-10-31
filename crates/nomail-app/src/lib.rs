//! Community application. HTTP depends on business modules, never the reverse.
//! Platform protocols live in nomail-core; deployments supply storage and runtime.
pub mod auth;
pub mod error;
pub mod http;
pub mod ingestion;
pub mod message;
pub mod pagination;
pub mod security;
pub mod webhook;

#[cfg(test)]
mod security_tests;
