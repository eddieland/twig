//! Configuration directory management for testing
//!
//! This module provides utilities for managing configuration directories
//! and registry files during testing.

use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow;

use crate::env::EnvTestGuard;

/// A reusable configuration directory structure for testing
pub struct ConfigDirsTestGuard {
  /// The configuration directory
  pub config_dir: PathBuf,
  /// The data directory
  pub data_dir: PathBuf,
  /// The cache directory (optional)
  pub cache_dir: Option<PathBuf>,
  /// The organization name used for paths
  pub organization: String,
  /// The application name used for paths
  pub application: String,
}

impl ConfigDirsTestGuard {
  /// Create a new TestConfigDirs instance with default organization and
  /// application names
  pub fn new() -> anyhow::Result<Self> {
    Self::with_names("eddieland", "", "twig")
  }

  /// Create a new TestConfigDirs instance with custom organization and
  /// application names
  pub fn with_names(organization: &str, _qualifier: &str, application: &str) -> anyhow::Result<Self> {
    // Get the XDG environment variables
    let config_home =
      env::var(EnvTestGuard::XDG_CONFIG_HOME).map_err(|_| anyhow::anyhow!("XDG_CONFIG_HOME not set"))?;

    let data_home = env::var(EnvTestGuard::XDG_DATA_HOME).map_err(|_| anyhow::anyhow!("XDG_DATA_HOME not set"))?;

    let cache_home = env::var(EnvTestGuard::XDG_CACHE_HOME).ok();

    // Construct the application-specific paths
    let config_dir = PathBuf::from(config_home).join(format!("{organization}/{application}"));
    let data_dir = PathBuf::from(data_home).join(format!("{organization}/{application}"));
    let cache_dir = cache_home.map(|dir| PathBuf::from(dir).join(format!("{organization}/{application}")));

    Ok(Self {
      config_dir,
      data_dir,
      cache_dir,
      organization: organization.to_string(),
      application: application.to_string(),
    })
  }

  /// Initialize the configuration directories
  pub fn init(&self) -> anyhow::Result<()> {
    // Create the config directory and its parent directories
    fs::create_dir_all(&self.config_dir).map_err(|e| anyhow::anyhow!("Failed to create config directory: {e}"))?;

    // Create the data directory and its parent directories
    fs::create_dir_all(&self.data_dir).map_err(|e| anyhow::anyhow!("Failed to create data directory: {e}"))?;

    // Create the cache directory if it exists
    if let Some(cache_dir) = &self.cache_dir {
      fs::create_dir_all(cache_dir).map_err(|e| anyhow::anyhow!("Failed to create cache directory: {e}"))?;
    }

    Ok(())
  }

  /// Initialize the configuration directories and create an empty registry file
  pub fn init_with_registry(&self) -> anyhow::Result<()> {
    // First initialize the directories
    self.init()?;

    // Create an empty registry file if it doesn't exist
    let registry_path = self.registry_path();

    // Ensure the parent directory exists
    if let Some(parent) = registry_path.parent() {
      fs::create_dir_all(parent).map_err(|e| anyhow::anyhow!("Failed to create registry parent directory: {e}"))?;
    }

    // Write the empty registry file
    fs::write(&registry_path, "[]").map_err(|e| anyhow::anyhow!("Failed to create empty registry file: {e}"))?;

    Ok(())
  }

  /// Get the path to the registry file
  pub fn registry_path(&self) -> PathBuf {
    self.data_dir.join("registry.json")
  }

  /// Verify that the configuration directories are in the expected location
  pub fn verify_in_test_env(&self, test_env: &EnvTestGuard) -> bool {
    // Canonicalize paths to handle symlinks (especially on macOS)
    let test_env_path =
      std::fs::canonicalize(test_env.temp_dir.path()).unwrap_or_else(|_| test_env.temp_dir.path().to_path_buf());
    let config_dir = std::fs::canonicalize(&self.config_dir).unwrap_or_else(|_| self.config_dir.clone());
    let data_dir = std::fs::canonicalize(&self.data_dir).unwrap_or_else(|_| self.data_dir.clone());

    // Check if the directories are within the test environment
    let config_in_test = config_dir.starts_with(&test_env_path);
    let data_in_test = data_dir.starts_with(&test_env_path);

    let cache_in_test = match &self.cache_dir {
      Some(cache_dir) => {
        let cache_dir = std::fs::canonicalize(cache_dir).unwrap_or_else(|_| cache_dir.clone());
        cache_dir.starts_with(&test_env_path)
      }
      None => true,
    };

    config_in_test && data_in_test && cache_in_test
  }

  /// Verify that the registry file exists and contains the expected content
  pub fn verify_registry(&self, expected_content: &str) -> anyhow::Result<bool> {
    let registry_path = self.registry_path();
    if !Path::new(&registry_path).exists() {
      return Ok(false);
    }

    let registry_content =
      fs::read_to_string(registry_path).map_err(|e| anyhow::anyhow!("Failed to read registry file: {e}"))?;

    Ok(registry_content == expected_content)
  }
}

/// A helper function to set up a test environment with TestConfigDirs
pub fn setup_test_env() -> anyhow::Result<(EnvTestGuard, ConfigDirsTestGuard)> {
  // Set up the test environment with overridden XDG directories
  let test_env = EnvTestGuard::new();

  // Create a TestConfigDirs instance, which should use our overridden XDG
  // directories
  let config_dirs = ConfigDirsTestGuard::new()?;

  Ok((test_env, config_dirs))
}

/// A helper function to set up a test environment with TestConfigDirs and
/// initialize it
pub fn setup_test_env_with_init() -> anyhow::Result<(EnvTestGuard, ConfigDirsTestGuard)> {
  // Set up the test environment with overridden XDG directories
  let test_env = EnvTestGuard::new();

  // Create a TestConfigDirs instance with paths directly in the test environment
  let config_dir = test_env.temp_dir.path().join("config/ai/twig");
  let data_dir = test_env.temp_dir.path().join("data/ai/twig");
  let cache_dir = Some(test_env.temp_dir.path().join("cache/ai/twig"));

  let config_dirs = ConfigDirsTestGuard {
    config_dir,
    data_dir,
    cache_dir,
    organization: "ai".to_string(),
    application: "twig".to_string(),
  };

  // Initialize the config directories
  config_dirs.init()?;

  Ok((test_env, config_dirs))
}

/// A helper function to set up a test environment with TestConfigDirs and
/// initialize it with a registry
pub fn setup_test_env_with_registry() -> anyhow::Result<(EnvTestGuard, ConfigDirsTestGuard)> {
  // Set up the test environment with overridden XDG directories
  let test_env = EnvTestGuard::new();

  // Create a TestConfigDirs instance with paths directly in the test environment
  let config_dir = test_env.temp_dir.path().join("config/ai/twig");
  let data_dir = test_env.temp_dir.path().join("data/ai/twig");
  let cache_dir = Some(test_env.temp_dir.path().join("cache/ai/twig"));

  let config_dirs = ConfigDirsTestGuard {
    config_dir,
    data_dir,
    cache_dir,
    organization: "ai".to_string(),
    application: "twig".to_string(),
  };

  // Initialize the config directories with registry
  config_dirs.init_with_registry()?;

  Ok((test_env, config_dirs))
}
