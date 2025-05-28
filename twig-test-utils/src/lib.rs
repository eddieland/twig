//! Test utilities shared across the twig workspace
//!
//! This crate provides common testing infrastructure including:
//! - XDG directory mocking ([`TestEnv`])
//! - Configuration directory testing ([`TestConfigDirs`])
//! - HOME directory isolation ([`TestHomeEnv`])
//!
//! The clippy dead_code lint is disabled for this crate because test utilities
//! may not be used by all tests, and the compiler cannot detect usage across
//! crate boundaries in development dependencies.

#![allow(clippy::dead_code)]

pub mod config;
pub mod env;
pub mod home;

// Re-export commonly used items
pub use config::{TestConfigDirs, setup_test_env, setup_test_env_with_init, setup_test_env_with_registry};
pub use env::TestEnv;
pub use home::TestHomeEnv;
