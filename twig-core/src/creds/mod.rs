//! # Credential Management
//!
//! Secure storage and retrieval of authentication credentials for external
//! services like GitHub and Jira, with support for multiple storage backends.
//!
//! This module provides cross-platform credential management with
//! platform-specific implementations for Unix (.netrc) and Windows (Windows
//! Credential Manager).

use std::path::Path;

use anyhow::Result;

use crate::creds::netrc::normalize_host;
use crate::creds::platform::CredentialProvider;

pub mod netrc;

// Platform-specific implementations
pub mod platform;
use platform::get_credential_provider;

/// Represents credentials for a service
#[derive(Debug, Clone)]
pub struct Credentials {
  pub username: String,
  pub password: String,
}

/// Check if Jira credentials are available
pub fn check_jira_credentials(home: &Path, jira_host: &str) -> Result<bool> {
  Ok(get_jira_credentials(home, jira_host).is_ok())
}

pub fn get_jira_credentials(home: &Path, jira_host: &str) -> Result<Credentials> {
  // Get the platform-specific credential provider
  let provider = get_credential_provider(home);

  let normalized_host = normalize_host(jira_host);
  if let Some(creds) = provider.get_credentials(&normalized_host)? {
    return Ok(creds);
  }
  // Try atlassian.net
  if let Some(creds) = provider.get_credentials("atlassian.net")? {
    return Ok(creds);
  }
  // Construct error message with already normalized host
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

/// Check if GitHub credentials are available
pub fn check_github_credentials(home: &Path) -> Result<bool> {
  let provider = get_credential_provider(home);
  let creds = provider.get_credentials("github.com")?;
  Ok(creds.is_some())
}

/// Get GitHub credentials
pub fn get_github_credentials(home: &Path) -> Result<Credentials> {
  let provider = get_credential_provider(home);
  match provider.get_credentials("github.com")? {
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

#[cfg(test)]
mod tests {
  use twig_test_utils::NetrcGuard;

  use super::*;
  use crate::consts::ENV_JIRA_HOST;

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

    // Test with JIRA_HOST set to non-existent host (should fall back to
    // atlassian.net)
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
    assert!(!error.contains("atlassian.com")); // This is no longer in the error message

    // Test check_jira_credentials with empty .netrc
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

    // Test with JIRA_HOST set to custom host
    unsafe {
      std::env::set_var(ENV_JIRA_HOST, "custom-jira-host.com");
    }
    assert!(check_jira_credentials(guard.home_dir(), "custom-jira-host.com").unwrap());

    // Test with JIRA_HOST set to non-existent host (should fall back to
    // atlassian.net)
    unsafe {
      std::env::set_var(ENV_JIRA_HOST, "nonexistent-host.com");
    }
    assert!(check_jira_credentials(guard.home_dir(), "nonexistent-host.com").unwrap());
  }

  #[test]
  fn test_check_jira_credentials_with_empty_netrc() {
    let guard = NetrcGuard::new("");

    assert!(!check_jira_credentials(guard.home_dir(), "custom-jira-host.com").unwrap());
  }

  #[test]
  fn test_get_github_credentials() {
    let content = r#"machine github.com
  login testuser
  password gh-token
"#;
    let guard = NetrcGuard::new(content);

    // Test getting GitHub credentials
    let github_creds = get_github_credentials(guard.home_dir()).unwrap();
    assert_eq!(github_creds.username, "testuser");
    assert_eq!(github_creds.password, "gh-token");
  }

  #[test]
  fn test_get_github_credentials_error() {
    let guard = NetrcGuard::new("");

    // Test error when GitHub credentials are missing
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

    // Test checking GitHub credentials
    assert!(check_github_credentials(guard.home_dir()).unwrap());
  }

  #[test]
  fn test_check_github_credentials_with_empty_netrc() {
    let guard = NetrcGuard::new("");

    // Test with empty .netrc
    assert!(!check_github_credentials(guard.home_dir()).unwrap());
  }

  #[test]
  #[cfg(unix)]
  fn test_netrc_permission_checking() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let guard = NetrcGuard::new(content);
    let netrc_path = guard.netrc_path().to_path_buf();

    // Set insecure permissions (readable by group/others)
    let mut perms = fs::metadata(&netrc_path).unwrap().permissions();
    perms.set_mode(0o644); // Insecure: readable by group and others
    fs::set_permissions(&netrc_path, perms).unwrap();

    // Check permissions
    let metadata = fs::metadata(&netrc_path).unwrap();
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    // Should detect insecure permissions
    assert_ne!(mode & 0o077, 0, "Expected insecure permissions to be detected");

    // Fix permissions
    let mut secure_perms = permissions;
    secure_perms.set_mode(0o600);
    fs::set_permissions(&netrc_path, secure_perms).unwrap();

    // Verify secure permissions
    let metadata = fs::metadata(&netrc_path).unwrap();
    let permissions = metadata.permissions();
    let mode = permissions.mode();

    assert_eq!(mode & 0o077, 0, "Expected secure permissions after fix");
  }

  #[test]
  fn test_credential_validation_scenarios() {
    // Test empty username/password
    let empty_creds = Credentials {
      username: "".to_string(),
      password: "".to_string(),
    };
    assert!(empty_creds.username.is_empty());
    assert!(empty_creds.password.is_empty());

    // Test valid credentials structure
    let valid_creds = Credentials {
      username: "testuser".to_string(),
      password: "testpass".to_string(),
    };
    assert!(!valid_creds.username.is_empty());
    assert!(!valid_creds.password.is_empty());
    assert_eq!(valid_creds.username, "testuser");
    assert_eq!(valid_creds.password, "testpass");
  }

  #[test]
  fn test_normalize_host_removes_https_and_trailing_slash() {
    let result = normalize_host("https://api.example.com/");
    assert_eq!(result, "api.example.com");
  }

  #[test]
  fn test_normalize_host_removes_http_and_trailing_slash() {
    let result = normalize_host("http://localhost:8080/");
    assert_eq!(result, "localhost:8080");
  }
}
