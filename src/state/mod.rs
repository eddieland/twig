use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::config::ConfigDirs;

/// Represents a repository in the registry
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Repository {
  pub path: String,
  pub name: String,
  pub last_fetch: Option<String>,
}

impl Repository {
  /// Create a new Repository instance
  pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
    let path_buf = fs::canonicalize(path.as_ref()).context("Failed to resolve repository path")?;

    let name = path_buf
      .file_name()
      .and_then(|n| n.to_str())
      .unwrap_or("unknown")
      .to_string();

    Ok(Self {
      path: path_buf.to_string_lossy().to_string(),
      name,
      last_fetch: None,
    })
  }
}

/// Represents the registry of tracked repositories
#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
  repositories: Vec<Repository>,
}

impl Registry {
  /// Load the registry from disk
  pub fn load(config_dirs: &ConfigDirs) -> Result<Self> {
    let registry_path = config_dirs.registry_path();

    if !registry_path.exists() {
      return Ok(Self {
        repositories: Vec::new(),
      });
    }

    let content = fs::read_to_string(&registry_path).context("Failed to read registry file")?;

    let repositories = serde_json::from_str(&content).context("Failed to parse registry file")?;

    Ok(Self { repositories })
  }

  /// Save the registry to disk
  pub fn save(&self, config_dirs: &ConfigDirs) -> Result<()> {
    let registry_path = config_dirs.registry_path();
    let content = serde_json::to_string_pretty(&self.repositories).context("Failed to serialize registry")?;

    fs::write(&registry_path, content).context("Failed to write registry file")?;

    Ok(())
  }

  /// Add a repository to the registry
  pub fn add<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
    let repo = Repository::new(path)?;

    // Check if the repository is already in the registry
    if self.repositories.iter().any(|r| r.path == repo.path) {
      println!("Repository already in registry: {}", repo.path);
      return Ok(());
    }

    self.repositories.push(repo);
    Ok(())
  }

  /// Remove a repository from the registry
  pub fn remove<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
    let path_buf = fs::canonicalize(path.as_ref()).context("Failed to resolve repository path")?;
    let path_str = path_buf.to_string_lossy().to_string();

    let initial_len = self.repositories.len();
    self.repositories.retain(|r| r.path != path_str);

    if self.repositories.len() == initial_len {
      println!("Repository not found in registry: {}", path_str);
    } else {
      println!("Removed repository from registry: {}", path_str);
    }

    Ok(())
  }

  /// List all repositories in the registry
  pub fn list(&self) -> &[Repository] {
    &self.repositories
  }

  /// Update the last fetch time for a repository
  pub fn update_fetch_time<P: AsRef<Path>>(&mut self, path: P, time: String) -> Result<()> {
    let path_buf = fs::canonicalize(path.as_ref()).context("Failed to resolve repository path")?;
    let path_str = path_buf.to_string_lossy().to_string();

    for repo in &mut self.repositories {
      if repo.path == path_str {
        repo.last_fetch = Some(time);
        return Ok(());
      }
    }

    Err(anyhow::anyhow!("Repository not found in registry: {}", path_str))
  }
}
