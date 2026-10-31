//! Community integration configuration and policies over the platform protocols.
//! Re-export protocol types so deployment adapters only need the application crate.
pub use nomail_core::webhook::*;
pub mod config;
pub mod service;
