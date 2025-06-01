//! Environment variable management for testing
//!
//! This module provides utilities for managing XDG environment variables
//! during testing to ensure tests don't interfere with each other.

use std::env;
use std::path::PathBuf;

use tempfile::TempDir;

/// A test environment that overrides XDG directories to use a per-test
/// temporary directory
pub struct EnvTestGuard {
  /// The temporary directory that will be used for XDG directories
  pub temp_dir: TempDir,
  /// The original XDG_CONFIG_HOME value, if any
  original_config_home: Option<String>,
  /// The original XDG_DATA_HOME value, if any
  original_data_home: Option<String>,
  /// The original XDG_CACHE_HOME value, if any
  original_cache_home: Option<String>,
}

impl Default for EnvTestGuard {
  fn default() -> Self {
    Self::new()
  }
}

impl EnvTestGuard {
  /// XDG environment variable names
  pub const XDG_CONFIG_HOME: &'static str = "XDG_CONFIG_HOME";
  pub const XDG_DATA_HOME: &'static str = "XDG_DATA_HOME";
  pub const XDG_CACHE_HOME: &'static str = "XDG_CACHE_HOME";

  /// Create a new test environment with overridden XDG directories
  pub fn new() -> Self {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");

    // Save original XDG environment variables
    let original_config_home = env::var(Self::XDG_CONFIG_HOME).ok();
    let original_data_home = env::var(Self::XDG_DATA_HOME).ok();
    let original_cache_home = env::var(Self::XDG_CACHE_HOME).ok();

    // Override XDG environment variables to use the temporary directory
    let temp_path = temp_dir.path().to_path_buf();
    unsafe {
      env::set_var(Self::XDG_CONFIG_HOME, temp_path.join("config"));
      env::set_var(Self::XDG_DATA_HOME, temp_path.join("data"));
      env::set_var(Self::XDG_CACHE_HOME, temp_path.join("cache"));
    }

    // Create the XDG directories
    std::fs::create_dir_all(temp_path.join("config")).expect("Failed to create config directory");
    std::fs::create_dir_all(temp_path.join("data")).expect("Failed to create data directory");
    std::fs::create_dir_all(temp_path.join("cache")).expect("Failed to create cache directory");

    Self {
      temp_dir,
      original_config_home,
      original_data_home,
      original_cache_home,
    }
  }

  /// Get the path to the XDG config directory
  pub fn config_dir(&self) -> PathBuf {
    self.temp_dir.path().join("config")
  }

  /// Get the path to the XDG data directory
  pub fn data_dir(&self) -> PathBuf {
    self.temp_dir.path().join("data")
  }

  /// Get the path to the XDG cache directory
  pub fn cache_dir(&self) -> PathBuf {
    self.temp_dir.path().join("cache")
  }
}

impl Drop for EnvTestGuard {
  fn drop(&mut self) {
    // Restore original XDG environment variables
    match &self.original_config_home {
      Some(val) => unsafe {
        env::set_var(EnvTestGuard::XDG_CONFIG_HOME, val);
      },
      None => unsafe {
        env::remove_var(EnvTestGuard::XDG_CONFIG_HOME);
      },
    }

    match &self.original_data_home {
      Some(val) => unsafe {
        env::set_var(EnvTestGuard::XDG_DATA_HOME, val);
      },
      None => unsafe {
        env::remove_var(EnvTestGuard::XDG_DATA_HOME);
      },
    }

    match &self.original_cache_home {
      Some(val) => unsafe {
        env::set_var(EnvTestGuard::XDG_CACHE_HOME, val);
      },
      None => unsafe {
        env::remove_var(EnvTestGuard::XDG_CACHE_HOME);
      },
    }
  }
}
