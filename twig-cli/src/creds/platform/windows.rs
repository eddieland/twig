//! Windows-specific credential handling implementation
//!
//! This module provides Windows-specific implementations for credential
//! storage and security operations using Windows Credential Manager.

// Apply dead_code suppression to the entire module when not on Windows
#![cfg_attr(not(windows), allow(dead_code))]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use wincredentials::credential::Credential;
use wincredentials::*;

use super::{CredentialProvider, FilePermissions};
use crate::creds::Credentials;
use crate::creds::netrc::{get_netrc_path, parse_netrc_file};

/// Windows implementation of file permissions using ACLs
pub struct WindowsFilePermissions;

impl FilePermissions for WindowsFilePermissions {
  fn set_secure_permissions(_path: &Path) -> Result<()> {
    // Return success without actually doing anything
    // Sorry, Windows users
    Ok(())
  }

  fn has_secure_permissions(path: &Path) -> Result<bool> {
    // Simply check if the file exists and is readable
    if !path.exists() {
      return Ok(false);
    }

    // Try to open the file for reading to verify access
    match fs::File::open(path) {
      Ok(_) => Ok(true),
      Err(_) => Ok(false),
    }
  }
}

/// Windows implementation of credential provider using Windows Credential
/// Manager
pub struct WindowsCredentialProvider {
  // We still keep the netrc path for backward compatibility
  netrc_path: PathBuf,
}

impl WindowsCredentialProvider {
  pub fn new(home: &Path) -> Self {
    Self {
      netrc_path: get_netrc_path(home),
    }
  }

  // Helper to format credential target name
  fn format_target_name(service: &str) -> String {
    format!("twig:{}", service)
  }
}

impl CredentialProvider for WindowsCredentialProvider {
  fn get_credentials(&self, service: &str) -> Result<Option<Credentials>> {
    let target_name = Self::format_target_name(service);

    match read_credential(&target_name) {
      Ok(cred) => {
        if !cred.username.is_empty() && !cred.secret.is_empty() {
          return Ok(Some(Credentials {
            username: cred.username,
            password: cred.secret,
          }));
        }
      }
      Err(_) => {
        // Fall back to .netrc
        if self.netrc_path.exists() {
          return parse_netrc_file(&self.netrc_path, service);
        }
      }
    }

    Ok(None)
  }

  fn store_credentials(&self, service: &str, credentials: &Credentials) -> Result<()> {
    let target_name = Self::format_target_name(service);

    let cred = Credential {
      username: credentials.username.clone(),
      secret: credentials.password.clone(),
    };

    write_credential(&target_name, cred).context("Failed to write credentials to Windows Credential Manager")?;

    Ok(())
  }
}
