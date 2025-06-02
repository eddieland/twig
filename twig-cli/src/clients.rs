//! # Client Creation
//!
//! Centralized client creation for external services like GitHub and Jira.
//! This module provides helper functions to create authenticated clients
//! with proper error handling and credential management.

// Re-export the original client creation functions
use anyhow::{Context, Result};
use tokio::runtime::Runtime;
use twig_gh::GitHubClient;
pub use twig_gh::create_github_client;
use twig_jira::JiraClient;
pub use twig_jira::create_jira_client;

use crate::consts::{DEFAULT_JIRA_HOST, ENV_JIRA_HOST};
use crate::creds::{get_github_credentials, get_jira_credentials};

/// Creates an authenticated GitHub client using credentials from .netrc
///
/// This function handles retrieving GitHub credentials and creating a client
/// in one step, with proper error handling.
pub fn create_github_client_from_netrc() -> Result<GitHubClient> {
  let credentials = get_github_credentials().context("Failed to get GitHub credentials")?;

  create_github_client(&credentials.username, &credentials.password).context("Failed to create GitHub client")
}

/// Creates an authenticated Jira client using credentials from .netrc
///
/// This function handles retrieving Jira credentials, determining the
/// Jira host URL, and creating a client in one step, with proper error
/// handling.
pub fn create_jira_client_from_netrc() -> Result<JiraClient> {
  let credentials = get_jira_credentials().context("Failed to get Jira credentials")?;

  // Get Jira host from environment or use default
  let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

  create_jira_client(&jira_host, &credentials.username, &credentials.password).context("Failed to create Jira client")
}

/// Creates a tokio runtime and an authenticated GitHub client
///
/// This is a convenience function for CLI commands that need both a runtime
/// and a GitHub client.
pub fn create_github_runtime_and_client() -> Result<(Runtime, GitHubClient)> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let client = create_github_client_from_netrc()?;
  Ok((rt, client))
}

/// Creates a tokio runtime and an authenticated Jira client
///
/// This is a convenience function for CLI commands that need both a runtime
/// and a Jira client.
pub fn create_jira_runtime_and_client() -> Result<(Runtime, JiraClient)> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let client = create_jira_client_from_netrc()?;
  Ok((rt, client))
}
