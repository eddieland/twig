//! HOME directory management for testing
//!
//! This module provides utilities for isolating HOME directory during testing
//! to prevent tests from interfering with the user's actual home directory.

use std::env;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// A test environment that overrides the HOME directory to use a temporary
/// directory This is useful for testing credential management and other
/// home directory dependent functionality
pub struct TestHomeEnv {
  /// The temporary directory that will be used as HOME
  pub temp_dir: TempDir,
  /// The original HOME value
  original_home: String,
}

impl TestHomeEnv {
  /// Create a new test environment with a temporary HOME directory
  pub fn new() -> Self {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");

    // Save original HOME environment variable
    let original_home = env::var("HOME").expect("HOME environment variable must be set");

    // Override HOME to use the temporary directory
    unsafe {
      env::set_var("HOME", temp_dir.path());
    }

    Self {
      temp_dir,
      original_home,
    }
  }

  /// Get the path to the temporary HOME directory
  pub fn home_dir(&self) -> &Path {
    self.temp_dir.path()
  }

  /// Get the path to a file in the temporary HOME directory
  pub fn home_path(&self, relative_path: &str) -> PathBuf {
    self.temp_dir.path().join(relative_path)
  }
}

impl Drop for TestHomeEnv {
  fn drop(&mut self) {
    // Restore original HOME environment variable
    unsafe {
      env::set_var("HOME", &self.original_home);
    }
  }
}
