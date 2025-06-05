//! # Client Creation
//!
//! Centralized client creation for external services like GitHub and Jira.
//! This module provides helper functions to create authenticated clients
//! with proper error handling and credential management.

use std::path::Path;

use anyhow::{Context, Result};
use tokio::runtime::Runtime;
pub use twig_gh::{GitHubClient, create_github_client};
pub use twig_jira::{JiraClient, create_jira_client};

use crate::consts::ENV_JIRA_HOST;
use crate::creds::{get_github_credentials, get_jira_credentials};

/// Get the $JIRA_HOST environment variable value
pub fn get_jira_host() -> Result<String> {
  let jira_host = std::env::var(ENV_JIRA_HOST);
  match jira_host {
    Ok(host) => Ok(host),
    Err(_) => Err(anyhow::anyhow!(
      "Jira host environment variable '{}' not set",
      ENV_JIRA_HOST
    )),
  }
}

/// Creates an authenticated GitHub client using credentials from .netrc
///
/// This function handles retrieving GitHub credentials and creating a client
/// in one step, with proper error handling.
pub fn create_github_client_from_netrc(home: &Path) -> Result<GitHubClient> {
  let credentials = get_github_credentials(home).context("Failed to get credentials")?;

  Ok(create_github_client(&credentials.username, &credentials.password))
}

/// Creates a tokio runtime and an authenticated GitHub client
///
/// This is a convenience function for CLI commands that need both a runtime
/// and a GitHub client.
pub fn create_github_runtime_and_client(home: &Path) -> Result<(Runtime, GitHubClient)> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let client = create_github_client_from_netrc(home)?;
  Ok((rt, client))
}

/// Creates an authenticated Jira client using credentials from .netrc
///
/// This function handles retrieving Jira credentials, determining the
/// Jira host URL, and creating a client in one step, with proper error
/// handling.
pub fn create_jira_client_from_netrc(home: &Path, jira_host: &str) -> Result<JiraClient> {
  let credentials = get_jira_credentials(home, jira_host).context("Failed to get credentials")?;

  Ok(create_jira_client(
    jira_host,
    &credentials.username,
    &credentials.password,
  ))
}

/// Creates a tokio runtime and an authenticated Jira client
///
/// This is a convenience function for CLI commands that need both a runtime
/// and a Jira client.
pub fn create_jira_runtime_and_client(home: &Path, jira_host: &str) -> Result<(Runtime, JiraClient)> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let client = create_jira_client_from_netrc(home, jira_host)?;
  Ok((rt, client))
}
