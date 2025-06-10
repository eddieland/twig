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
use url::Url;

use crate::consts::ENV_JIRA_HOST;
use crate::creds::{get_github_credentials, get_jira_credentials};

/// Ensure a host URL has a scheme, defaulting to https:// if none is present
fn ensure_scheme(host: &str) -> Result<String> {
  let trimmed = host.trim();
  if trimmed.is_empty() {
    return Err(anyhow::anyhow!("Host cannot be empty"));
  }

  // Try to parse as URL first
  match Url::parse(trimmed) {
    Ok(url) => {
      // If it parses successfully with any scheme, return normalized URL without
      // trailing slash
      let mut result = url.to_string();
      // Remove trailing slash if it's just the root path
      if result.ends_with('/') && url.path() == "/" {
        result.pop();
      }
      Ok(result)
    }
    Err(_) => {
      // If parsing fails, try adding https:// prefix
      let with_scheme = format!("https://{trimmed}");
      match Url::parse(&with_scheme) {
        Ok(url) => {
          let mut result = url.to_string();
          // Remove trailing slash if it's just the root path
          if result.ends_with('/') && url.path() == "/" {
            result.pop();
          }
          Ok(result)
        }
        Err(e) => Err(anyhow::anyhow!("Invalid URL: {}", e)),
      }
    }
  }
}

/// Get the $JIRA_HOST environment variable value
/// If the host doesn't include a scheme (http:// or https://), assumes https://
pub fn get_jira_host() -> Result<String> {
  let jira_host = std::env::var(ENV_JIRA_HOST);
  match jira_host {
    Ok(host) => Ok(ensure_scheme(&host)?),
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ensure_scheme_with_https() {
    let result = ensure_scheme("https://company.atlassian.net").unwrap();
    assert_eq!(result, "https://company.atlassian.net");
  }

  #[test]
  fn test_ensure_scheme_with_http() {
    let result = ensure_scheme("http://jira.example.com").unwrap();
    assert_eq!(result, "http://jira.example.com");
  }

  #[test]
  fn test_ensure_scheme_without_scheme() {
    let result = ensure_scheme("company.atlassian.net").unwrap();
    assert_eq!(result, "https://company.atlassian.net");
  }

  #[test]
  fn test_ensure_scheme_with_subdomain() {
    let result = ensure_scheme("my-company.atlassian.net").unwrap();
    assert_eq!(result, "https://my-company.atlassian.net");
  }

  #[test]
  fn test_ensure_scheme_empty_string() {
    let result = ensure_scheme("");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Host cannot be empty"));
  }

  #[test]
  fn test_ensure_scheme_whitespace_only() {
    let result = ensure_scheme("   ");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Host cannot be empty"));
  }

  #[test]
  fn test_ensure_scheme_with_port() {
    let result = ensure_scheme("localhost:8080").unwrap();
    assert_eq!(result, "localhost:8080"); // URL crate parses this as scheme "localhost"
  }

  #[test]
  fn test_ensure_scheme_with_http_and_port() {
    let result = ensure_scheme("http://localhost:8080").unwrap();
    assert_eq!(result, "http://localhost:8080");
  }

  #[test]
  fn test_ensure_scheme_with_https_and_port() {
    let result = ensure_scheme("https://localhost:9443").unwrap();
    assert_eq!(result, "https://localhost:9443");
  }

  #[test]
  fn test_ensure_scheme_ip_address() {
    let result = ensure_scheme("192.168.1.100").unwrap();
    assert_eq!(result, "https://192.168.1.100");
  }

  #[test]
  fn test_ensure_scheme_ip_address_with_port() {
    let result = ensure_scheme("192.168.1.100:8080").unwrap();
    assert_eq!(result, "https://192.168.1.100:8080");
  }

  #[test]
  fn test_ensure_scheme_localhost() {
    let result = ensure_scheme("localhost").unwrap();
    assert_eq!(result, "https://localhost");
  }

  #[test]
  fn test_ensure_scheme_with_path() {
    let result = ensure_scheme("example.com/path/to/resource").unwrap();
    assert_eq!(result, "https://example.com/path/to/resource");
  }

  #[test]
  fn test_ensure_scheme_with_query_params() {
    let result = ensure_scheme("example.com?param=value").unwrap();
    assert_eq!(result, "https://example.com/?param=value"); // URL crate adds / before query
  }

  #[test]
  fn test_ensure_scheme_with_fragment() {
    let result = ensure_scheme("example.com#section").unwrap();
    assert_eq!(result, "https://example.com/#section"); // URL crate adds / before fragment
  }

  #[test]
  fn test_ensure_scheme_case_sensitivity() {
    let result = ensure_scheme("HTTP://example.com").unwrap();
    assert_eq!(result, "http://example.com"); // URL crate normalizes scheme to lowercase
  }

  #[test]
  fn test_ensure_scheme_case_sensitivity_https() {
    let result = ensure_scheme("HTTPS://example.com").unwrap();
    assert_eq!(result, "https://example.com"); // URL crate normalizes scheme to lowercase
  }

  #[test]
  fn test_ensure_scheme_partial_scheme_http() {
    let result = ensure_scheme("http:/example.com").unwrap();
    assert_eq!(result, "http://example.com"); // URL crate normalizes by adding missing slash
  }

  #[test]
  fn test_ensure_scheme_partial_scheme_https() {
    let result = ensure_scheme("https:/example.com").unwrap();
    assert_eq!(result, "https://example.com"); // URL crate normalizes by adding missing slash
  }

  #[test]
  fn test_ensure_scheme_scheme_in_middle() {
    let result = ensure_scheme("example.com/http://other.com").unwrap();
    assert_eq!(result, "https://example.com/http://other.com");
  }

  #[test]
  fn test_ensure_scheme_ftp_protocol() {
    let result = ensure_scheme("ftp://example.com").unwrap();
    assert_eq!(result, "ftp://example.com"); // Any valid scheme should be preserved
  }

  #[test]
  fn test_get_jira_host_with_scheme() {
    unsafe {
      std::env::set_var(ENV_JIRA_HOST, "https://test.atlassian.net");
    }

    let result = get_jira_host().unwrap();
    assert_eq!(result, "https://test.atlassian.net");
  }

  #[test]
  fn test_get_jira_host_without_scheme() {
    unsafe {
      std::env::set_var(ENV_JIRA_HOST, "test.atlassian.net");
    }

    let result = get_jira_host().unwrap();
    assert_eq!(result, "https://test.atlassian.net");
  }

  #[test]
  fn test_get_jira_host_not_set() {
    unsafe {
      std::env::remove_var(ENV_JIRA_HOST);
    }

    let result = get_jira_host();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not set"));
  }
}
