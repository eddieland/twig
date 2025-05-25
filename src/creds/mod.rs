use anyhow::{Context, Result};
use directories::BaseDirs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

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

/// Check if the .netrc file exists
#[allow(dead_code)]
pub fn netrc_exists() -> bool {
  get_netrc_path().exists()
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
#[allow(dead_code)]
pub fn get_github_credentials() -> Result<Credentials> {
  match parse_netrc_for_machine("github.com")? {
    Some(creds) => Ok(creds),
    None => Err(anyhow::anyhow!(
      "GitHub credentials not found in .netrc file. Please add credentials for machine 'github.com'."
    )),
  }
}
