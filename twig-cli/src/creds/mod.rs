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
  let creds = parse_netrc_for_machine("atlassian.com")?;
  Ok(creds.is_some())
}

/// Get Jira credentials
pub fn get_jira_credentials() -> Result<Credentials> {
  match parse_netrc_for_machine("atlassian.com")? {
    Some(creds) => Ok(creds),
    None => Err(anyhow::anyhow!(
      "Jira credentials not found in .netrc file. Please add credentials for machine 'atlassian.com'."
    )),
  }
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
  use std::fs;
  use std::io::Write;

  use tempfile::TempDir;

  use super::*;

  /// Helper function to create a test .netrc file
  fn create_test_netrc(content: &str) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    let mut file = fs::File::create(&netrc_path).expect("Failed to create test .netrc");
    file.write_all(content.as_bytes()).expect("Failed to write test .netrc");

    (temp_dir, netrc_path)
  }

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

  /// Helper function to write netrc entry to a specific path (for testing)
  fn write_netrc_entry_to_path(path: &Path, machine: &str, username: &str, password: &str) -> Result<()> {
    // Read existing content if file exists
    let mut existing_content = String::new();
    let mut machine_exists = false;

    if path.exists() {
      existing_content = std::fs::read_to_string(path).context("Failed to read existing .netrc file")?;

      // Check if machine already exists
      machine_exists = existing_content.contains(&format!("machine {}", machine));
    }

    if machine_exists {
      // Update existing entry
      let lines: Vec<&str> = existing_content.lines().collect();
      let mut new_content = String::new();
      let mut skip_until_next_machine = false;

      for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with("machine ") {
          if trimmed == format!("machine {}", machine) {
            skip_until_next_machine = true;
            // Add the updated machine entry
            new_content.push_str(&format!("machine {}\n", machine));
            new_content.push_str(&format!("  login {}\n", username));
            new_content.push_str(&format!("  password {}\n", password));
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

      writeln!(file, "machine {}", machine)?;
      writeln!(file, "  login {}", username)?;
      writeln!(file, "  password {}", password)?;
    }

    Ok(())
  }
}
