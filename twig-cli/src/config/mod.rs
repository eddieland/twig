//! # Configuration Management
//!
//! Handles application configuration, directory management, and settings
//! for the twig tool, including XDG base directory support.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

/// Represents the configuration directories for the twig application
pub struct ConfigDirs {
  pub config_dir: PathBuf,
  pub data_dir: PathBuf,
  pub cache_dir: Option<PathBuf>,
}

impl ConfigDirs {
  /// Create a new ConfigDirs instance
  pub fn new() -> Result<Self> {
    let proj_dirs = ProjectDirs::from("ai", "lat", "twig").context("Failed to determine project directories")?;

    let config_dir = proj_dirs.config_dir().to_path_buf();
    let data_dir = proj_dirs.data_dir().to_path_buf();
    let cache_dir = Some(proj_dirs.cache_dir().to_path_buf());

    Ok(Self {
      config_dir,
      data_dir,
      cache_dir,
    })
  }

  /// Get the config directory
  pub fn config_dir(&self) -> &PathBuf {
    &self.config_dir
  }

  /// Get the data directory
  pub fn data_dir(&self) -> &PathBuf {
    &self.data_dir
  }

  /// Get the cache directory
  pub fn cache_dir(&self) -> Option<&PathBuf> {
    self.cache_dir.as_ref()
  }

  /// Initialize the configuration directories
  pub fn init(&self) -> Result<()> {
    fs::create_dir_all(&self.config_dir).context("Failed to create config directory")?;
    fs::create_dir_all(&self.data_dir).context("Failed to create data directory")?;

    // Create an empty registry file if it doesn't exist
    let registry_path = self.registry_path();
    if !registry_path.exists() {
      fs::write(&registry_path, "[]").context("Failed to create empty registry file")?;
    }

    Ok(())
  }

  /// Get the path to the registry file
  pub fn registry_path(&self) -> PathBuf {
    self.data_dir.join("registry.json")
  }

  /// Get the path to the repository-local state directory
  pub fn repo_state_dir<P: AsRef<Path>>(&self, repo_path: P) -> PathBuf {
    repo_path.as_ref().join(".twig")
  }

  /// Get the path to the repository-local state file
  pub fn repo_state_path<P: AsRef<Path>>(&self, repo_path: P) -> PathBuf {
    self.repo_state_dir(repo_path).join("state.json")
  }
}

/// Initialize the configuration directories
pub fn init() -> Result<()> {
  use crate::utils::output::{format_repo_path, print_success};

  let config_dirs = ConfigDirs::new()?;
  config_dirs.init()?;

  print_success("Initialized twig configuration directories:");
  println!(
    "  Config: {}",
    format_repo_path(&config_dirs.config_dir.display().to_string())
  );
  println!(
    "  Data: {}",
    format_repo_path(&config_dirs.data_dir.display().to_string())
  );

  Ok(())
}

/// Get the configuration directories
pub fn get_config_dirs() -> Result<ConfigDirs> {
  ConfigDirs::new()
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;
  use twig_test_utils::TestEnv;

  use super::*;

  #[test]
  fn test_config_dirs_creation() {
    // Use TestEnv from twig-test-utils to properly manage environment variables
    let _test_env = TestEnv::new();

    let config_dirs = ConfigDirs::new().unwrap();
    let _ = config_dirs.init();

    assert!(config_dirs.config_dir().exists() || config_dirs.config_dir().parent().unwrap().exists());
    assert!(config_dirs.data_dir().exists() || config_dirs.data_dir().parent().unwrap().exists());
    assert!(config_dirs.cache_dir().is_some());
  }

  #[test]
  fn test_registry_path() {
    let _test_env = TestEnv::new();

    let config_dirs = ConfigDirs::new().unwrap();
    let registry_path = config_dirs.registry_path();

    assert!(registry_path.ends_with("registry.json"));
    assert!(registry_path.starts_with(config_dirs.data_dir()));
  }

  #[test]
  fn test_repo_state_paths() {
    let _test_env = TestEnv::new();

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    let config_dirs = ConfigDirs::new().unwrap();

    let state_dir = config_dirs.repo_state_dir(repo_path);
    let state_path = config_dirs.repo_state_path(repo_path);

    assert_eq!(state_dir, repo_path.join(".twig"));
    assert_eq!(state_path, repo_path.join(".twig/state.json"));
  }

  #[test]
  fn test_init_creates_directories() {
    let _test_env = TestEnv::new();

    let config_dirs = ConfigDirs::new().unwrap();
    config_dirs.init().unwrap();

    assert!(config_dirs.config_dir().exists());
    assert!(config_dirs.data_dir().exists());
    assert!(config_dirs.registry_path().exists());
  }
}
