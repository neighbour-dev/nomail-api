//! Business protocols shared by server and edge deployments.
//!
//! No HTTP framework, database, clock, executor or environment access is required.
#![doc = include_str!("../README.md")]
pub mod api;
pub mod auth;
pub mod error;
pub mod message;
pub mod pagination;
pub mod security;
#[cfg(test)]
mod security_tests;
pub mod webhook;
