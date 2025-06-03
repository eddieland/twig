//! Platform-specific credential handling implementations
//!
//! This module provides platform-specific implementations for credential
//! storage and security operations.

use std::path::Path;

use anyhow::Result;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

/// Trait for platform-specific file permission operations
pub trait FilePermissions {
  /// Set secure permissions on a credential file
  fn set_secure_permissions(path: &Path) -> Result<()>;

  /// Check if a file has secure permissions
  fn has_secure_permissions(path: &Path) -> Result<bool>;
}

/// Trait for platform-specific credential storage operations
pub trait CredentialProvider {
  /// Get credentials for a service
  fn get_credentials(&self, service: &str) -> Result<Option<crate::creds::Credentials>>;

  /// Store credentials for a service\
  #[allow(dead_code)]
  fn store_credentials(&self, service: &str, credentials: &crate::creds::Credentials) -> Result<()>;
}

/// Get the appropriate credential provider for the current platform
pub fn get_credential_provider() -> Box<dyn CredentialProvider> {
  #[cfg(unix)]
  {
    Box::new(unix::NetrcCredentialProvider::new())
  }
  #[cfg(windows)]
  {
    Box::new(windows::WindowsCredentialProvider::new())
  }
}
