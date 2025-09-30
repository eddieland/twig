//! Windows-specific credential handling implementation
//!
//! This module provides Windows-specific implementations for credential
//! storage and security operations using Windows Credential Manager.

// Apply dead_code suppression to the entire module when not on Windows
#![cfg_attr(not(windows), allow(dead_code))]

use std::ffi::c_void;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use windows_sys::Win32::Foundation::{GetLastError, ERROR_NOT_FOUND, FILETIME};
use windows_sys::Win32::Security::Credentials::{
  CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

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
    format!("twig:{service}")
  }
}

impl CredentialProvider for WindowsCredentialProvider {
  fn get_credentials(&self, service: &str) -> Result<Option<Credentials>> {
    let target_name = Self::format_target_name(service);

    match read_windows_credential(&target_name) {
      Ok(Some(cred)) => {
        if !cred.username.is_empty() && !cred.password.is_empty() {
          return Ok(Some(cred));
        }
      }
      Ok(None) => {
        if self.netrc_path.exists() {
          return parse_netrc_file(&self.netrc_path, service);
        }
      }
      Err(error) => {
        // Only fall back to netrc when the credential isn't present. Propagate other
        // errors so users can surface unexpected Credential Manager failures.
        if self.netrc_path.exists() {
          tracing::warn!(
            error = %error,
            target = %target_name,
            "Falling back to .netrc after Windows Credential Manager error",
          );
          return parse_netrc_file(&self.netrc_path, service);
        }
        return Err(error);
      }
    }

    Ok(None)
  }

  fn store_credentials(&self, service: &str, credentials: &Credentials) -> Result<()> {
    let target_name = Self::format_target_name(service);

    write_windows_credential(&target_name, credentials)
      .context("Failed to write credentials to Windows Credential Manager")?;

    Ok(())
  }
}

fn to_utf16_null_terminated(value: &str) -> Vec<u16> {
  std::ffi::OsStr::new(value)
    .encode_wide()
    .chain(std::iter::once(0))
    .collect()
}

fn read_windows_credential(target_name: &str) -> Result<Option<Credentials>> {
  let wide_target = to_utf16_null_terminated(target_name);
  let mut credential_ptr: *mut CREDENTIALW = std::ptr::null_mut();

  let status = unsafe { CredReadW(wide_target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential_ptr) };
  if status == 0 {
    let error = unsafe { GetLastError() };
    if error == ERROR_NOT_FOUND {
      return Ok(None);
    }

    return Err(anyhow!("CredReadW failed with error code {error}"));
  }

  if credential_ptr.is_null() {
    return Ok(None);
  }

  let credential = CredentialHandle::new(credential_ptr);
  let username = unsafe { wide_ptr_to_string(credential.user_name())? };
  let secret = {
    let blob_ptr = credential.credential_blob();
    let blob_size = credential.credential_blob_size();
    if blob_size == 0 || blob_ptr.is_null() {
      String::new()
    } else {
      let blob = unsafe { std::slice::from_raw_parts(blob_ptr, blob_size as usize) };
      String::from_utf8(blob.to_vec()).context("Credential secret is not valid UTF-8")?
    }
  };

  Ok(Some(Credentials {
    username,
    password: secret,
  }))
}

unsafe fn wide_ptr_to_string(ptr: *const u16) -> Result<String> {
  if ptr.is_null() {
    return Ok(String::new());
  }

  let mut len = 0usize;
  while *ptr.add(len) != 0 {
    len += 1;
  }

  let slice = std::slice::from_raw_parts(ptr, len);
  String::from_utf16(slice).context("Failed to convert UTF-16 string")
}

fn write_windows_credential(target_name: &str, credentials: &Credentials) -> Result<()> {
  let mut wide_target = to_utf16_null_terminated(target_name);
  let mut wide_username = to_utf16_null_terminated(&credentials.username);
  let mut secret_bytes = credentials.password.as_bytes().to_vec();

  let mut credential = CREDENTIALW {
    Flags: 0,
    Type: CRED_TYPE_GENERIC,
    TargetName: wide_target.as_mut_ptr(),
    Comment: std::ptr::null_mut(),
    LastWritten: FILETIME {
      dwLowDateTime: 0,
      dwHighDateTime: 0,
    },
    CredentialBlobSize: secret_bytes.len() as u32,
    CredentialBlob: if secret_bytes.is_empty() {
      std::ptr::null_mut()
    } else {
      secret_bytes.as_mut_ptr()
    },
    Persist: CRED_PERSIST_LOCAL_MACHINE,
    AttributeCount: 0,
    Attributes: std::ptr::null_mut(),
    TargetAlias: std::ptr::null_mut(),
    UserName: wide_username.as_mut_ptr(),
  };

  let status = unsafe { CredWriteW(&credential, 0) };
  if status == 0 {
    let error = unsafe { GetLastError() };
    return Err(anyhow!("CredWriteW failed with error code {error}"));
  }

  Ok(())
}

struct CredentialHandle(*mut CREDENTIALW);

impl CredentialHandle {
  fn new(ptr: *mut CREDENTIALW) -> Self {
    Self(ptr)
  }

  fn user_name(&self) -> *const u16 {
    unsafe { self.0.as_ref() }
      .map(|cred| cred.UserName as *const u16)
      .unwrap_or(std::ptr::null::<u16>())
  }

  fn credential_blob(&self) -> *const u8 {
    unsafe { self.0.as_ref() }
      .map(|cred| cred.CredentialBlob as *const u8)
      .unwrap_or(std::ptr::null::<u8>())
  }

  fn credential_blob_size(&self) -> u32 {
    unsafe { self.0.as_ref() }
      .map(|cred| cred.CredentialBlobSize)
      .unwrap_or(0)
  }
}

impl Drop for CredentialHandle {
  fn drop(&mut self) {
    if !self.0.is_null() {
      unsafe { CredFree(self.0 as *mut c_void) };
    }
  }
}
