//! Authentication helpers for the GitHub client.
//!
//! This module provides convenience functions for loading credentials from the
//! user's environment and creating ready-to-use GitHub clients. The helpers are
//! shared between the CLI and plugins so that all consumers can rely on the
//! same credential discovery logic.

use std::path::Path;

use anyhow::{Context, Result};
use tokio::runtime::Runtime;
use twig_core::creds::Credentials;
use twig_core::creds::platform::{CredentialProvider, get_credential_provider};

use crate::{GitHubClient, create_github_client};

const GITHUB_MACHINE: &str = "github.com";

/// Check if GitHub credentials are available for the current user.
pub fn check_github_credentials(home: &Path) -> Result<bool> {
  let provider = get_credential_provider(home);
  let creds = provider.get_credentials(GITHUB_MACHINE)?;
  Ok(creds.is_some())
}

/// Load GitHub credentials from the configured credential provider.
pub fn get_github_credentials(home: &Path) -> Result<Credentials> {
  let provider = get_credential_provider(home);
  match provider.get_credentials(GITHUB_MACHINE)? {
    Some(creds) => Ok(creds),
    None => {
      #[cfg(unix)]
      let error_msg = "GitHub credentials not found in .netrc file. Please add credentials for machine 'github.com'.";
      #[cfg(windows)]
      let error_msg =
        "GitHub credentials not found. Please run 'twig creds setup' to configure credentials for 'github.com'.";
      Err(anyhow::anyhow!(error_msg))
    }
  }
}

/// Creates an authenticated GitHub client using credentials from .netrc.
pub fn create_github_client_from_netrc(home: &Path) -> Result<GitHubClient> {
  let credentials = get_github_credentials(home).context("Failed to get credentials")?;

  Ok(create_github_client(&credentials.username, &credentials.password))
}

/// Creates a tokio runtime and an authenticated GitHub client.
pub fn create_github_runtime_and_client(home: &Path) -> Result<(Runtime, GitHubClient)> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let client = create_github_client_from_netrc(home)?;
  Ok((rt, client))
}

#[cfg(test)]
mod tests {
  use twig_test_utils::NetrcGuard;

  use super::*;

  #[test]
  fn test_get_github_credentials() {
    let content = r#"machine github.com
  login testuser
  password gh-token
"#;
    let guard = NetrcGuard::new(content);

    let github_creds = get_github_credentials(guard.home_dir()).unwrap();
    assert_eq!(github_creds.username, "testuser");
    assert_eq!(github_creds.password, "gh-token");
  }

  #[test]
  fn test_get_github_credentials_error() {
    let guard = NetrcGuard::new("");

    let error = get_github_credentials(guard.home_dir()).unwrap_err().to_string();
    assert!(error.contains("GitHub credentials not found"));
    assert!(error.contains("github.com"));
  }

  #[test]
  fn test_check_github_credentials() {
    let content = r#"machine github.com
  login testuser
  password gh-token
"#;

    let guard = NetrcGuard::new(content);
    assert!(check_github_credentials(guard.home_dir()).unwrap());
  }

  #[test]
  fn test_check_github_credentials_with_empty_netrc() {
    let guard = NetrcGuard::new("");

    assert!(!check_github_credentials(guard.home_dir()).unwrap());
  }
}
