use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::BaseDirs;
use tempfile::TempDir;

/// Get the path to the .netrc file
fn get_netrc_path() -> PathBuf {
  let base_dirs = BaseDirs::new().expect("Could not determine base directories");
  let home = base_dirs.home_dir();
  home.join(".netrc")
}

/// RAII guard for test .netrc files
///
/// This struct creates a temporary .netrc file with the given content, sets
/// the HOME environment variable to point to the temporary directory, and
/// restores the original HOME environment variable when dropped.
pub struct NetrcGuard {
  #[allow(dead_code)]
  temp_dir: TempDir,
  netrc_path: PathBuf,
  original_home: PathBuf,
}

impl NetrcGuard {
  /// Create a new NetrcGuard with the given content
  pub fn new(content: &str) -> Self {
    // Save original home path
    let original_home = get_netrc_path().parent().unwrap().to_path_buf();

    // Create temporary directory and .netrc file
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let netrc_path = temp_dir.path().join(".netrc");

    let mut file = fs::File::create(&netrc_path).expect("Failed to create test .netrc");
    file.write_all(content.as_bytes()).expect("Failed to write test .netrc");

    // Set HOME environment variable to the temporary directory
    unsafe {
      std::env::set_var("HOME", temp_dir.path());
    }

    Self {
      temp_dir,
      netrc_path,
      original_home,
    }
  }

  /// Get the path to the .netrc file
  pub fn netrc_path(&self) -> &Path {
    &self.netrc_path
  }

  /// Get the path to the temporary directory
  pub fn home_dir(&self) -> &Path {
    self.temp_dir.path()
  }
}

impl Drop for NetrcGuard {
  fn drop(&mut self) {
    // Restore original HOME environment variable
    unsafe {
      std::env::set_var("HOME", &self.original_home);
    }
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
