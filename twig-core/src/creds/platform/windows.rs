//! Windows-specific credential handling implementation
//!
//! This module provides Windows-specific implementations for credential
//! storage and security operations using Windows Credential Manager.

// Apply dead_code suppression to the entire module when not on Windows
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::{fs, ptr};

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
use windows_sys::Win32::Security::Credentials::{
  CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW, CredWriteW,
};

use super::{CredentialProvider, FilePermissions};
use crate::creds::Credentials;
use crate::creds::netrc::{get_netrc_path, parse_netrc_file};

mod helpers;
use helpers::{pwstr_to_string, to_wide};

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
    format!("twig:{service}")
  }
}

impl CredentialProvider for WindowsCredentialProvider {
  fn get_credentials(&self, service: &str) -> Result<Option<Credentials>> {
    let target_name = Self::format_target_name(service);

    match read_windows_credential(&target_name) {
      Ok(Some(creds)) => return Ok(Some(creds)),
      Ok(None) | Err(_) => {
        if self.netrc_path.exists() {
          return parse_netrc_file(&self.netrc_path, service);
        }
      }
    }

    Ok(None)
  }

  fn store_credentials(&self, service: &str, credentials: &Credentials) -> Result<()> {
    let target_name = Self::format_target_name(service);
    write_windows_credential(&target_name, credentials)
  }
}

fn read_windows_credential(target_name: &str) -> Result<Option<Credentials>> {
  let target_name_wide = to_wide(target_name);
  let mut credential_ptr: *mut CREDENTIALW = ptr::null_mut();

  let read_result = unsafe { CredReadW(target_name_wide.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential_ptr) };

  if read_result == 0 {
    let error = unsafe { GetLastError() };
    if error == ERROR_NOT_FOUND {
      return Ok(None);
    }

    bail!("CredReadW failed with error code {:#x}", error);
  }

  if credential_ptr.is_null() {
    return Ok(None);
  }

  let credential = unsafe { &*credential_ptr };

  let username = unsafe { pwstr_to_string(credential.UserName) };
  let password_bytes =
    unsafe { std::slice::from_raw_parts(credential.CredentialBlob, credential.CredentialBlobSize as usize) };
  let password = String::from_utf8_lossy(password_bytes).into_owned();

  unsafe {
    CredFree(credential_ptr as *const c_void);
  }

  if username.is_empty() || password.is_empty() {
    return Ok(None);
  }

  Ok(Some(Credentials { username, password }))
}

#[allow(dead_code)]
fn write_windows_credential(target_name: &str, credentials: &Credentials) -> Result<()> {
  let mut target_name_wide = to_wide(target_name);
  let mut username_wide = to_wide(&credentials.username);
  let password_bytes = credentials.password.as_bytes();

  let credential = CREDENTIALW {
    Flags: 0,
    Type: CRED_TYPE_GENERIC,
    TargetName: target_name_wide.as_mut_ptr(),
    Comment: ptr::null_mut(),
    LastWritten: unsafe { std::mem::zeroed() },
    CredentialBlobSize: password_bytes.len() as u32,
    CredentialBlob: password_bytes.as_ptr() as *mut u8,
    Persist: CRED_PERSIST_LOCAL_MACHINE,
    AttributeCount: 0,
    Attributes: ptr::null_mut(),
    TargetAlias: ptr::null_mut(),
    UserName: username_wide.as_mut_ptr(),
  };

  let write_result = unsafe { CredWriteW(&credential, 0) };

  if write_result == 0 {
    let error = unsafe { GetLastError() };
    bail!("CredWriteW failed with error code {:#x}", error);
  }

  Ok(())
}
