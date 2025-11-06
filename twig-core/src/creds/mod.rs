//! # Credential Management
//!
//! Secure storage and retrieval of authentication credentials for external
//! services like GitHub and Jira, with support for multiple storage backends.
//!
//! This module provides cross-platform credential management with
//! platform-specific implementations for Unix (.netrc) and Windows (Windows
//! Credential Manager).

pub mod netrc;

// Platform-specific implementations
pub mod platform;

/// Represents credentials for a service
#[derive(Debug, Clone)]
pub struct Credentials {
  pub username: String,
  pub password: String,
}
