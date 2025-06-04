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

#![allow(dead_code)]

pub mod config;
pub mod env;
pub mod git;
pub mod home;
pub mod netrc;

// Re-export commonly used items
pub use config::{ConfigDirsTestGuard, setup_test_env, setup_test_env_with_init, setup_test_env_with_registry};
pub use env::EnvTestGuard;
pub use git::GitRepoTestGuard;
pub use home::HomeEnvTestGuard;
pub use netrc::NetrcGuard;
