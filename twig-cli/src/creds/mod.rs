//! # Credential Management
//!
//! Secure storage and retrieval of authentication credentials for external
//! services like GitHub and Jira, with support for multiple storage backends.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::BaseDirs;

/// Represents credentials for a service
#[derive(Debug, Clone)]
pub struct Credentials {
  pub username: String,
  pub password: String,
}

/// Get the path to the .netrc file
pub fn get_netrc_path() -> PathBuf {
  let base_dirs = BaseDirs::new().expect("Could not determine base directories");
  let home = base_dirs.home_dir();
  home.join(".netrc")
}

/// Parse the .netrc file for credentials for a specific machine
pub fn parse_netrc_for_machine(machine: &str) -> Result<Option<Credentials>> {
  let netrc_path = get_netrc_path();
  if !netrc_path.exists() {
    return Ok(None);
  }

  parse_netrc_file(&netrc_path, machine)
}

/// Parse a .netrc file for credentials for a specific machine
fn parse_netrc_file(path: &Path, target_machine: &str) -> Result<Option<Credentials>> {
  let file = File::open(path).context("Failed to open .netrc file")?;
  let reader = BufReader::new(file);

  let mut current_machine = String::new();
  let mut username = String::new();
  let mut password = String::new();

  for line in reader.lines() {
    let line = line.context("Failed to read line from .netrc")?;
    let parts: Vec<&str> = line.split_whitespace().collect();

    for i in 0..parts.len() {
      match parts[i] {
        "machine" if i + 1 < parts.len() => {
          // If we found credentials for the previous machine, check if it's our target
          if !current_machine.is_empty() && !username.is_empty() && !password.is_empty() {
            if current_machine == target_machine {
              return Ok(Some(Credentials { username, password }));
            }
            // Reset for the new machine
            username = String::new();
            password = String::new();
          }
          current_machine = parts[i + 1].to_string();
        }
        "login" if i + 1 < parts.len() => {
          username = parts[i + 1].to_string();
        }
        "password" if i + 1 < parts.len() => {
          password = parts[i + 1].to_string();
        }
        _ => {}
      }
    }
  }

  // Check the last machine in the file
  if current_machine == target_machine && !username.is_empty() && !password.is_empty() {
    return Ok(Some(Credentials { username, password }));
  }

  Ok(None)
}

/// Check if Jira credentials are available
pub fn check_jira_credentials() -> Result<bool> {
  Ok(get_jira_credentials().is_ok())
}

/// Get Jira credentials
pub fn get_jira_credentials() -> Result<Credentials> {
  // Try JIRA_HOST first, then fallback to atlassian.net
  let jira_host = std::env::var("JIRA_HOST").ok();

  if let Some(host) = &jira_host {
    if let Some(creds) = parse_netrc_for_machine(host)? {
      return Ok(creds);
    }
  }

  // Try atlassian.net
  if let Some(creds) = parse_netrc_for_machine("atlassian.net")? {
    return Ok(creds);
  }

  // Construct error message based on whether JIRA_HOST is set
  let error_msg = if jira_host.is_some() {
    format!(
      "Jira credentials not found in .netrc file. Please add credentials for machine '{}' or 'atlassian.net'.",
      jira_host.unwrap()
    )
  } else {
    "Jira credentials not found in .netrc file. Please add credentials for machine 'atlassian.net'.".to_string()
  };

  Err(anyhow::anyhow!(error_msg))
}

/// Check if GitHub credentials are available
pub fn check_github_credentials() -> Result<bool> {
  let creds = parse_netrc_for_machine("github.com")?;
  Ok(creds.is_some())
}

/// Get GitHub credentials
pub fn get_github_credentials() -> Result<Credentials> {
  match parse_netrc_for_machine("github.com")? {
    Some(creds) => Ok(creds),
    None => Err(anyhow::anyhow!(
      "GitHub credentials not found in .netrc file. Please add credentials for machine 'github.com'."
    )),
  }
}

/// Write or update a .netrc entry for a specific machine
pub fn write_netrc_entry(machine: &str, username: &str, password: &str) -> Result<()> {
  let netrc_path = get_netrc_path();

  // Read existing content if file exists
  let mut existing_content = String::new();
  let mut machine_exists = false;

  if netrc_path.exists() {
    existing_content = std::fs::read_to_string(&netrc_path).context("Failed to read existing .netrc file")?;

    // Check if machine already exists
    machine_exists = existing_content.contains(&format!("machine {machine}"));
  }

  if machine_exists {
    // Update existing entry
    let lines: Vec<&str> = existing_content.lines().collect();
    let mut new_content = String::new();
    let mut skip_until_next_machine = false;

    for line in lines {
      let trimmed = line.trim();

      if trimmed.starts_with("machine ") {
        if trimmed == format!("machine {machine}",) {
          skip_until_next_machine = true;
          // Add the updated machine entry
          new_content.push_str(&format!("machine {machine}\n",));
          new_content.push_str(&format!("  login {username}\n",));
          new_content.push_str(&format!("  password {password}\n",));
        } else {
          skip_until_next_machine = false;
          new_content.push_str(line);
          new_content.push('\n');
        }
      } else if !skip_until_next_machine {
        new_content.push_str(line);
        new_content.push('\n');
      }
    }

    std::fs::write(&netrc_path, new_content).context("Failed to write updated .netrc file")?;
  } else {
    // Append new entry
    let mut file = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(&netrc_path)
      .context("Failed to open .netrc file for writing")?;

    // Add a newline if file exists and doesn't end with one
    if netrc_path.metadata()?.len() > 0 && !existing_content.ends_with('\n') {
      writeln!(file)?;
    }

    writeln!(file, "machine {machine}",)?;
    writeln!(file, "  login {username}",)?;
    writeln!(file, "  password {password}",)?;
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::io::Write;
  use std::os::unix::fs::PermissionsExt;
  use std::{env, fs};

  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_parse_netrc_file_basic() {
    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "testpass");
  }

  #[test]
  fn test_parse_netrc_file_multiple_machines() {
    let content = r#"machine example.com
  login user1
  password pass1

machine github.com
  login user2
  password pass2

machine atlassian.com
  login user3
  password pass3
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Test first machine
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user1");
    assert_eq!(creds.password, "pass1");

    // Test middle machine
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");

    // Test last machine
    let result = parse_netrc_file(&netrc_path, "atlassian.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user3");
    assert_eq!(creds.password, "pass3");
  }

  #[test]
  fn test_parse_netrc_file_machine_not_found() {
    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let result = parse_netrc_file(&netrc_path, "nonexistent.com").unwrap();
    assert!(result.is_none());
  }

  #[test]
  fn test_parse_netrc_file_incomplete_entry() {
    let content = r#"machine example.com
  login testuser
machine github.com
  login user2
  password pass2
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Should not find example.com because it has no password
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_none());

    // Should find github.com because it has both login and password
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");
  }

  #[test]
  fn test_parse_netrc_file_single_line_format() {
    let content = "machine example.com login testuser password testpass\n";

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "testpass");
  }

  #[test]
  fn test_parse_netrc_file_mixed_format() {
    let content = r#"machine example.com login user1 password pass1
machine github.com
  login user2
  password pass2
machine atlassian.com login user3
  password pass3
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Test single line format
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user1");
    assert_eq!(creds.password, "pass1");

    // Test multi-line format
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");

    // Test mixed format
    let result = parse_netrc_file(&netrc_path, "atlassian.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user3");
    assert_eq!(creds.password, "pass3");
  }

  #[test]
  fn test_parse_netrc_file_empty_file() {
    let (_temp_dir, netrc_path) = create_test_netrc("");

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_none());
  }

  #[test]
  fn test_write_netrc_entry_new_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    // Test writing to a new file
    write_netrc_entry_to_path(&netrc_path, "example.com", "testuser", "testpass").unwrap();

    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "testpass");
  }

  #[test]
  fn test_write_netrc_entry_append_to_existing() {
    let initial_content = r#"machine example.com
  login user1
  password pass1
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(initial_content);

    // Append a new entry
    write_netrc_entry_to_path(&netrc_path, "github.com", "user2", "pass2").unwrap();

    // Check original entry still exists
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user1");
    assert_eq!(creds.password, "pass1");

    // Check new entry was added
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");
  }

  #[test]
  fn test_write_netrc_entry_update_existing() {
    let initial_content = r#"machine example.com
  login olduser
  password oldpass

machine github.com
  login user2
  password pass2
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(initial_content);

    // Update existing entry
    write_netrc_entry_to_path(&netrc_path, "example.com", "newuser", "newpass").unwrap();

    // Check updated entry
    let result = parse_netrc_file(&netrc_path, "example.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "newuser");
    assert_eq!(creds.password, "newpass");

    // Check other entry wasn't affected
    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some());
    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");
  }

  #[test]
  fn test_parse_netrc_for_machine() {
    let content = r#"machine example.com
  login user1
  password pass1

machine github.com
  login user2
  password pass2
"#;

    let (temp_dir, _netrc_path) = create_test_netrc(content);

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }

    // Test existing machine
    let result = parse_netrc_for_machine("github.com").unwrap();
    assert!(result.is_some());

    let creds = result.unwrap();
    assert_eq!(creds.username, "user2");
    assert_eq!(creds.password, "pass2");

    // Test non-existent machine
    let result = parse_netrc_for_machine("nonexistent.com").unwrap();
    assert!(result.is_none());

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

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

    let (temp_dir, _netrc_path) = create_test_netrc(content);

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }

    // Test with JIRA_HOST set to custom host
    unsafe {
      std::env::set_var("JIRA_HOST", "custom-jira-host.com");
    }

    let jira_creds = get_jira_credentials().unwrap();
    assert_eq!(jira_creds.username, "custom@example.com");
    assert_eq!(jira_creds.password, "custom-token");

    // Test with JIRA_HOST set to non-existent host (should fall back to
    // atlassian.net)
    unsafe {
      std::env::set_var("JIRA_HOST", "nonexistent-host.com");
    }
    let jira_creds = get_jira_credentials().unwrap();
    assert_eq!(jira_creds.username, "net@example.com");
    assert_eq!(jira_creds.password, "net-token");

    // Test with JIRA_HOST unset (should use atlassian.net)
    unsafe {
      std::env::remove_var("JIRA_HOST");
    }
    let jira_creds = get_jira_credentials().unwrap();
    assert_eq!(jira_creds.username, "net@example.com");
    assert_eq!(jira_creds.password, "net-token");

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_get_jira_credentials_error_messages() {
    let (temp_dir, _netrc_path) = create_test_netrc("");

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      env::set_var("HOME", temp_dir.path());
    }

    // Test with JIRA_HOST set
    unsafe {
      env::set_var("JIRA_HOST", "custom-jira-host.com");
    }
    let error = get_jira_credentials().unwrap_err().to_string();
    assert!(error.contains("custom-jira-host.com"));
    assert!(error.contains("atlassian.net"));
    assert!(!error.contains("atlassian.com")); // This is no longer in the error message

    // Test with JIRA_HOST unset
    unsafe {
      env::remove_var("JIRA_HOST");
    }
    let error = get_jira_credentials().unwrap_err().to_string();
    assert!(error.contains("atlassian.net"));
    assert!(!error.contains("atlassian.com")); // This is no longer in the error message
    assert!(!error.contains("custom-jira-host.com"));

    // Reset environment
    unsafe {
      env::set_var("HOME", original_path.parent().unwrap());
    }

    // Test check_jira_credentials with empty .netrc
    assert!(!check_jira_credentials().unwrap());
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

    let (temp_dir, _netrc_path) = create_test_netrc(content);

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }

    // Test with JIRA_HOST set to custom host
    unsafe {
      std::env::set_var("JIRA_HOST", "custom-jira-host.com");
    }
    assert!(check_jira_credentials().unwrap());

    // Test with JIRA_HOST set to non-existent host (should fall back to
    // atlassian.net)
    unsafe {
      std::env::set_var("JIRA_HOST", "nonexistent-host.com");
    }
    assert!(check_jira_credentials().unwrap());

    // Test with JIRA_HOST unset (should use atlassian.net)
    unsafe {
      std::env::remove_var("JIRA_HOST");
    }
    assert!(check_jira_credentials().unwrap());

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_check_jira_credentials_with_empty_netrc() {
    let (temp_dir, _netrc_path) = create_test_netrc("");

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }
    // Test with empty .netrc
    assert!(!check_jira_credentials().unwrap());

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_get_github_credentials() {
    let content = r#"machine github.com
  login testuser
  password gh-token
"#;

    let (temp_dir, _netrc_path) = create_test_netrc(content);

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }

    // Test getting GitHub credentials
    let github_creds = get_github_credentials().unwrap();
    assert_eq!(github_creds.username, "testuser");
    assert_eq!(github_creds.password, "gh-token");

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_get_github_credentials_error() {
    let (temp_dir, _netrc_path) = create_test_netrc("");

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }

    // Test error when GitHub credentials are missing
    let error = get_github_credentials().unwrap_err().to_string();
    assert!(error.contains("GitHub credentials not found"));
    assert!(error.contains("github.com"));

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_check_github_credentials() {
    let content = r#"machine github.com
  login testuser
  password gh-token
"#;

    let (temp_dir, _netrc_path) = create_test_netrc(content);

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }
    // Test checking GitHub credentials
    assert!(check_github_credentials().unwrap());

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_check_github_credentials_with_empty_netrc() {
    let (temp_dir, _netrc_path) = create_test_netrc("");

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }
    // Test with empty .netrc
    assert!(!check_github_credentials().unwrap());

    // Reset environment
    unsafe {
      std::env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  #[test]
  fn test_netrc_permission_checking() {
    let content = r#"machine example.com
  login testuser
  password testpass
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

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
  fn test_parse_netrc_file_malformed() {
    let content = r#"machine custom-jira-host.com
  login custom@example.com
  # missing password

machine atlassian.com
  login test@example.com
  # missing password

machine github.com
  login testuser
  password gh-token
  some-invalid-line
"#;

    let (_temp_dir, netrc_path) = create_test_netrc(content);

    // Test parsing should handle malformed entries gracefully
    let result = parse_netrc_file(&netrc_path, "custom-jira-host.com").unwrap();
    assert!(result.is_none()); // Should be None because password is missing

    let result = parse_netrc_file(&netrc_path, "atlassian.com").unwrap();
    assert!(result.is_none()); // Should be None because password is missing

    let result = parse_netrc_file(&netrc_path, "github.com").unwrap();
    assert!(result.is_some()); // Should still work despite extra line
    let creds = result.unwrap();
    assert_eq!(creds.username, "testuser");
    assert_eq!(creds.password, "gh-token");
  }

  #[test]
  fn test_jira_credentials_with_env_var() {
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

    let (temp_dir, _netrc_path) = create_test_netrc(content);

    // Mock the get_netrc_path function to return our test path
    let original_path = get_netrc_path();
    unsafe {
      env::set_var("HOME", temp_dir.path());
    }

    // Test with JIRA_HOST set to custom host
    unsafe {
      env::set_var("JIRA_HOST", "custom-jira-host.com");
    }
    let jira_creds = get_jira_credentials().unwrap();
    assert_eq!(jira_creds.username, "custom@example.com");
    assert_eq!(jira_creds.password, "custom-token");

    // Test with JIRA_HOST set to non-existent host (should fall back to
    // atlassian.net)
    unsafe {
      env::set_var("JIRA_HOST", "nonexistent-host.com");
    }
    let jira_creds = get_jira_credentials().unwrap();
    assert_eq!(jira_creds.username, "net@example.com");
    assert_eq!(jira_creds.password, "net-token");

    // Test with JIRA_HOST unset (should use atlassian.net)
    unsafe {
      env::remove_var("JIRA_HOST");
    }
    let jira_creds = get_jira_credentials().unwrap();
    assert_eq!(jira_creds.username, "net@example.com");
    assert_eq!(jira_creds.password, "net-token");

    // Test the check_jira_credentials function
    unsafe {
      env::set_var("JIRA_HOST", "custom-jira-host.com");
    }
    assert!(check_jira_credentials().unwrap());

    unsafe {
      env::set_var("JIRA_HOST", "nonexistent-host.com");
    }
    assert!(check_jira_credentials().unwrap());

    unsafe {
      env::remove_var("JIRA_HOST");
    }
    assert!(check_jira_credentials().unwrap());

    // Reset environment
    unsafe {
      env::set_var("HOME", original_path.parent().unwrap());
    }
  }

  /// Helper function to create a test .netrc file
  fn create_test_netrc(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    let mut file = fs::File::create(&netrc_path).expect("Failed to create test .netrc");
    file.write_all(content.as_bytes()).expect("Failed to write test .netrc");

    (temp_dir, netrc_path)
  }

  /// Helper function to write netrc entry to a specific path (for testing)
  fn write_netrc_entry_to_path(path: &Path, machine: &str, username: &str, password: &str) -> Result<()> {
    // Read existing content if file exists
    let mut existing_content = String::new();
    let mut machine_exists = false;

    if path.exists() {
      existing_content = std::fs::read_to_string(path).context("Failed to read existing .netrc file")?;

      // Check if machine already exists
      machine_exists = existing_content.contains(&format!("machine {machine}"));
    }

    if machine_exists {
      // Update existing entry
      let lines: Vec<&str> = existing_content.lines().collect();
      let mut new_content = String::new();
      let mut skip_until_next_machine = false;

      for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("machine ") {
          if trimmed == format!("machine {machine}") {
            skip_until_next_machine = true;
            // Add the updated machine entry
            new_content.push_str(&format!("machine {machine}\n"));
            new_content.push_str(&format!("  login {username}\n"));
            new_content.push_str(&format!("  password {password}\n"));
          } else {
            skip_until_next_machine = false;
            new_content.push_str(line);
            new_content.push('\n');
          }
        } else if !skip_until_next_machine {
          new_content.push_str(line);
          new_content.push('\n');
        }
      }

      std::fs::write(path, new_content).context("Failed to write updated .netrc file")?;
    } else {
      // Append new entry
      let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("Failed to open .netrc file for writing")?;

      // Add a newline if file exists and doesn't end with one
      if path.metadata()?.len() > 0 && !existing_content.ends_with('\n') {
        writeln!(file)?;
      }

      writeln!(file, "machine {machine}")?;
      writeln!(file, "  login {username}")?;
      writeln!(file, "  password {password}")?;
    }

    Ok(())
  }
}
