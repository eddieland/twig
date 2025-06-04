//! Unix-specific credential handling implementation
//!
//! This module provides Unix-specific implementations for credential
//! storage and security operations using .netrc files.

// Apply dead_code suppression to the entire module when on Windows
#![cfg_attr(windows, allow(dead_code))]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{CredentialProvider, FilePermissions};
use crate::creds::{Credentials, parse_netrc_file};

/// Unix implementation of file permissions using chmod-style permissions
pub struct UnixFilePermissions;

impl FilePermissions for UnixFilePermissions {
  fn set_secure_permissions(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path).context("Failed to get file metadata")?.permissions();
    perms.set_mode(0o600); // Owner read/write only
    fs::set_permissions(path, perms).context("Failed to set secure permissions")
  }

  fn has_secure_permissions(path: &Path) -> Result<bool> {
    let metadata = fs::metadata(path).context("Failed to get file metadata")?;
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    // Check if the file is only accessible by the owner (no group/other
    // permissions)
    Ok(mode & 0o077 == 0)
  }
}

/// Unix implementation of credential provider using .netrc files
pub struct NetrcCredentialProvider {
  netrc_path: PathBuf,
}

impl Default for NetrcCredentialProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl NetrcCredentialProvider {
  pub fn new() -> Self {
    Self {
      netrc_path: crate::creds::get_netrc_path(),
    }
  }
}

impl CredentialProvider for NetrcCredentialProvider {
  fn get_credentials(&self, service: &str) -> Result<Option<Credentials>> {
    if !self.netrc_path.exists() {
      return Ok(None);
    }

    parse_netrc_file(&self.netrc_path, service)
  }

  fn store_credentials(&self, service: &str, credentials: &Credentials) -> Result<()> {
    crate::creds::write_netrc_entry(service, &credentials.username, &credentials.password)?;

    // Ensure secure permissions
    UnixFilePermissions::set_secure_permissions(&self.netrc_path)?;

    Ok(())
  }
}
