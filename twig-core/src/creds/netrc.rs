//! Helpers for reading and writing credentials stored in `.netrc` files.
//!
//! These utilities are shared by the platform-specific credential providers and
//! keep parsing and serialization logic in one place so the CLI and service
//! clients can consistently discover credentials.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::creds::platform::FilePermissions;
use crate::creds::{Credentials, platform};

/// Returns the path to the `.netrc` file for the provided home directory.
///
/// # Arguments
///
/// * `home` - The user's home directory, typically from
///   `directories::BaseDirs::home_dir`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use twig_core::creds::netrc::get_netrc_path;
///
/// let home = Path::new("/home/user");
/// let path = get_netrc_path(home);
/// assert_eq!(path, Path::new("/home/user/.netrc"));
/// ```
pub fn get_netrc_path(home: &Path) -> PathBuf {
  home.join(".netrc")
}

/// Parses a `.netrc` file and returns credentials for the requested machine.
///
/// The parser supports both single-line (`machine host login user password pass`)
/// and multi-line formats. If the target machine is not present or has missing
/// `login`/`password` values, `Ok(None)` is returned.
///
/// # Arguments
///
/// * `path` - Path to the `.netrc` file to read.
/// * `target_machine` - Hostname to search for (e.g. `github.com`).
///
/// # Returns
///
/// * `Ok(Some(Credentials))` when valid credentials are found.
/// * `Ok(None)` when the machine entry is missing or incomplete.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
pub fn parse_netrc_file(path: &Path, target_machine: &str) -> Result<Option<Credentials>> {
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

/// Writes or updates a `.netrc` entry for the given machine.
///
/// Existing entries for the machine are replaced; otherwise a new entry is
/// appended. When writing on Unix platforms, the file permissions are tightened
/// to `600` to avoid exposing credentials.
///
/// # Arguments
///
/// * `path` - Location of the `.netrc` file.
/// * `machine` - Hostname to associate with the credentials.
/// * `username` - Login value to store.
/// * `password` - Password or token to store.
///
/// # Errors
///
/// Returns an error if the file cannot be read from or written to, or if
/// permissions cannot be set.
pub fn write_netrc_entry(path: &Path, machine: &str, username: &str, password: &str) -> Result<()> {
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

    writeln!(file, "machine {machine}",)?;
    writeln!(file, "  login {username}",)?;
    writeln!(file, "  password {password}",)?;
  }

  // Set secure permissions on the file
  #[cfg(unix)]
  {
    platform::UnixFilePermissions::set_secure_permissions(path)?;
  }

  #[cfg(windows)]
  {
    // note: this is a no-op on Windows, but we call it for consistency
    platform::WindowsFilePermissions::set_secure_permissions(path)?;
  }

  Ok(())
}

/// Normalizes a Jira host URL by removing protocol prefixes and trailing
/// slashes.
///
/// # Arguments
///
/// * `raw_host` - A string slice containing the raw host URL that may include
///   protocol prefixes (http:// or https://) and/or trailing slashes
///
/// # Returns
///
/// A `String` containing the normalized hostname without protocol or trailing
/// slash
///
/// # Examples
///
/// ```
/// let host1 = normalize_host("https://company.atlassian.net/");
/// assert_eq!(host1, "company.atlassian.net");
///
/// let host2 = normalize_host("http://jira.example.com");
/// assert_eq!(host2, "jira.example.com");
///
/// let host3 = normalize_host("my-jira-instance.com");
/// assert_eq!(host3, "my-jira-instance.com");
/// ```
pub fn normalize_host(raw_host: &str) -> String {
  raw_host
    .trim_start_matches("https://")
    .trim_start_matches("http://")
    .trim_end_matches('/')
    .to_string()
}

#[cfg(test)]
mod tests {
  use std::fs;
  use std::io::Write;

  use tempfile::TempDir;
  use twig_test_utils::NetrcGuard;

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
    write_netrc_entry(&netrc_path, "example.com", "testuser", "testpass").unwrap();

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
    write_netrc_entry(&netrc_path, "github.com", "user2", "pass2").unwrap();

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
    write_netrc_entry(&netrc_path, "example.com", "newuser", "newpass").unwrap();

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
  #[cfg(unix)]
  fn test_netrc_permission_checking() {
    use std::os::unix::fs::PermissionsExt;

    use twig_test_utils::NetrcGuard;

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
    let guard = NetrcGuard::new(content);
    let netrc_path = guard.netrc_path().to_path_buf();

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
  fn test_normalize_host_removes_https_and_trailing_slash() {
    let result = normalize_host("https://api.example.com/");
    assert_eq!(result, "api.example.com");
  }

  #[test]
  fn test_normalize_host_removes_http_and_trailing_slash() {
    let result = normalize_host("http://localhost:8080/");
    assert_eq!(result, "localhost:8080");
  }

  /// Helper function to create a test .netrc file
  fn create_test_netrc(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    let mut file = fs::File::create(&netrc_path).expect("Failed to create test .netrc");
    file.write_all(content.as_bytes()).expect("Failed to write test .netrc");

    (temp_dir, netrc_path)
  }
}
