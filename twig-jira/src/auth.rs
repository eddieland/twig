//! Authentication helpers for the Jira client.
//!
//! These helpers centralize credential lookup and runtime construction so that
//! both the CLI and plugins can reuse the same authentication flow when talking
//! to Jira.

use std::path::Path;

use anyhow::{Context, Result};
use tokio::runtime::Runtime;
use twig_core::creds::Credentials;
use twig_core::creds::netrc::normalize_host;
use twig_core::creds::platform::{CredentialProvider, get_credential_provider};
pub use twig_core::url::ENV_JIRA_HOST;
use twig_core::url::resolve_jira_base_url;

use crate::{JiraClient, create_jira_client};

/// Get the $JIRA_HOST environment variable value.
/// If the host doesn't include a scheme (http:// or https://), assumes https://.
pub fn get_jira_host() -> Result<String> {
  resolve_jira_base_url()
}

/// Check if Jira credentials are available for the provided host.
pub fn check_jira_credentials(home: &Path, jira_host: &str) -> Result<bool> {
  Ok(get_jira_credentials(home, jira_host).is_ok())
}

/// Retrieve Jira credentials from the configured credential provider.
pub fn get_jira_credentials(home: &Path, jira_host: &str) -> Result<Credentials> {
  let provider = get_credential_provider(home);

  let normalized_host = normalize_host(jira_host);
  if let Some(creds) = provider.get_credentials(&normalized_host)? {
    return Ok(creds);
  }
  if let Some(creds) = provider.get_credentials("atlassian.net")? {
    return Ok(creds);
  }

  #[cfg(unix)]
  let error_msg = format!(
    "Jira credentials not found in .netrc file. Please add credentials for machine '{normalized_host}' or 'atlassian.net'."
  );
  #[cfg(windows)]
  let error_msg = format!(
    "Jira credentials not found. Please run 'twig creds setup' to configure credentials for '{normalized_host}' or 'atlassian.net'."
  );
  Err(anyhow::anyhow!(error_msg))
}

/// Creates an authenticated Jira client using credentials from .netrc.
pub fn create_jira_client_from_netrc(home: &Path, jira_host: &str) -> Result<JiraClient> {
  let credentials = get_jira_credentials(home, jira_host).context("Failed to get credentials")?;

  Ok(create_jira_client(
    jira_host,
    &credentials.username,
    &credentials.password,
  ))
}

/// Creates a tokio runtime and an authenticated Jira client.
pub fn create_jira_runtime_and_client(home: &Path, jira_host: &str) -> Result<(Runtime, JiraClient)> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let client = create_jira_client_from_netrc(home, jira_host)?;
  Ok((rt, client))
}

#[cfg(test)]
mod tests {
  use twig_test_utils::{EnvVarGuard, NetrcGuard};

  use super::*;

  #[test]
  fn test_get_jira_credentials() {
    let content = r#"machine custom-jira-host.com
  login custom@example.com
  password custom-token

machine atlassian.com
  login test@example.com
  password test-token

machine atlassian.net
  login net@example.com
  password net-token
"#;
    let guard = NetrcGuard::new(content);

    let jira_creds = get_jira_credentials(guard.home_dir(), "custom-jira-host.com").unwrap();
    assert_eq!(jira_creds.username, "custom@example.com");
    assert_eq!(jira_creds.password, "custom-token");

    let jira_creds = get_jira_credentials(guard.home_dir(), "nonexistent-host.com").unwrap();
    assert_eq!(jira_creds.username, "net@example.com");
    assert_eq!(jira_creds.password, "net-token");
  }

  #[test]
  fn test_get_jira_credentials_error_messages() {
    let guard = NetrcGuard::new("");

    let error = get_jira_credentials(guard.home_dir(), "custom-jira-host.com")
      .unwrap_err()
      .to_string();
    assert!(error.contains("custom-jira-host.com"));
    assert!(error.contains("atlassian.net"));
    assert!(!error.contains("atlassian.com"));

    assert!(!check_jira_credentials(guard.home_dir(), "custom-jira-host.com").unwrap());
  }

  #[test]
  fn test_check_jira_credentials() {
    let content = r#"machine custom-jira-host.com
  login custom@example.com
  password custom-token

machine atlassian.com
  login test@example.com
  password test-token

machine atlassian.net
  login net@example.com
  password net-token
"#;
    let guard = NetrcGuard::new(content);

    let jira_host_guard = EnvVarGuard::new(ENV_JIRA_HOST);
    jira_host_guard.set("custom-jira-host.com");
    assert!(check_jira_credentials(guard.home_dir(), "custom-jira-host.com").unwrap());

    jira_host_guard.set("nonexistent-host.com");
    assert!(check_jira_credentials(guard.home_dir(), "nonexistent-host.com").unwrap());
  }

  #[test]
  fn test_check_jira_credentials_with_empty_netrc() {
    let guard = NetrcGuard::new("");

    assert!(!check_jira_credentials(guard.home_dir(), "custom-jira-host.com").unwrap());
  }
}
