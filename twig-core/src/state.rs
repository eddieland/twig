//! # Application State Management
//!
//! Manages global application state including repository registry,
//! workspace tracking, and persistent configuration across twig sessions.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
      return Ok(());
    }

    self.repositories.push(repo);
    Ok(())
  }

  /// Remove a repository from the registry
  pub fn remove<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
    let path_buf = fs::canonicalize(path.as_ref()).context("Failed to resolve repository path")?;
    let path_str = path_buf.to_string_lossy().to_string();

    self.repositories.retain(|r| r.path != path_str);
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

/// Represents a user-defined branch dependency
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchDependency {
  pub id: Uuid,
  pub child: String,
  pub parent: String,
  pub created_at: DateTime<Utc>,
}

/// Represents a user-defined root branch
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RootBranch {
  pub id: Uuid,
  pub branch: String,
  pub is_default: bool,
  pub created_at: DateTime<Utc>,
}

/// Represents a worktree in a repository
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Worktree {
  pub name: String,
  pub path: String,
  pub branch: String,
  pub created_at: String,
}

/// Represents a branch-issue association
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchMetadata {
  pub branch: String,
  pub jira_issue: Option<String>,
  pub github_pr: Option<u32>,
  pub created_at: String,
}

/// Represents the repository-local state
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RepoState {
  pub version: u32,
  pub updated_at: DateTime<Utc>,
  pub worktrees: Vec<Worktree>,
  pub branches: HashMap<String, BranchMetadata>,
  pub dependencies: Vec<BranchDependency>,
  pub root_branches: Vec<RootBranch>,

  // Pre-built indices for fast lookups (rebuilt on load, not saved)
  #[serde(skip)]
  pub branch_to_jira_index: HashMap<String, String>,
  #[serde(skip)]
  pub jira_to_branch_index: HashMap<String, String>,
  #[serde(skip)]
  pub dependency_children_index: HashMap<String, Vec<String>>,
  #[serde(skip)]
  pub dependency_parents_index: HashMap<String, Vec<String>>,
}

impl RepoState {
  /// Load the repository state from disk
  pub fn load<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
    let config_dirs = crate::config::ConfigDirs::new()?;
    let state_path = config_dirs.repo_state_path(&repo_path);

    if !state_path.exists() {
      let mut state = Self {
        version: 1,
        updated_at: Utc::now(),
        worktrees: Vec::new(),
        branches: HashMap::new(),
        dependencies: Vec::new(),
        root_branches: Vec::new(),
        branch_to_jira_index: HashMap::new(),
        jira_to_branch_index: HashMap::new(),
        dependency_children_index: HashMap::new(),
        dependency_parents_index: HashMap::new(),
      };
      state.rebuild_indices();
      return Ok(state);
    }

    let content = fs::read_to_string(&state_path).context("Failed to read state file")?;
    let mut state: Self = serde_json::from_str(&content).context("Failed to parse state file")?;

    // Rebuild indices after loading
    state.rebuild_indices();
    Ok(state)
  }

  /// Rebuild the lookup indices
  pub fn rebuild_indices(&mut self) {
    self.branch_to_jira_index.clear();
    self.jira_to_branch_index.clear();
    self.dependency_children_index.clear();
    self.dependency_parents_index.clear();

    // Build branch-jira indices
    for (branch, metadata) in &self.branches {
      if let Some(jira_issue) = &metadata.jira_issue {
        self.branch_to_jira_index.insert(branch.clone(), jira_issue.clone());
        self.jira_to_branch_index.insert(jira_issue.clone(), branch.clone());
      }
    }

    // Build dependency indices
    for dep in &self.dependencies {
      self
        .dependency_children_index
        .entry(dep.parent.clone())
        .or_default()
        .push(dep.child.clone());

      self
        .dependency_parents_index
        .entry(dep.child.clone())
        .or_default()
        .push(dep.parent.clone());
    }
  }

  /// Get children of a branch
  pub fn get_children(&self, branch: &str) -> Vec<&str> {
    self
      .dependency_children_index
      .get(branch)
      .map(|children| children.iter().map(|s| s.as_str()).collect())
      .unwrap_or_default()
  }

  /// Get parents of a branch
  pub fn get_parents(&self, branch: &str) -> Vec<&str> {
    self
      .dependency_parents_index
      .get(branch)
      .map(|parents| parents.iter().map(|s| s.as_str()).collect())
      .unwrap_or_default()
  }
}

#[cfg(test)]
mod tests {
  use std::fs;

  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_repository_creation() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let repo = Repository::new(repo_path).unwrap();

    assert!(!repo.path.is_empty());
    assert!(!repo.name.is_empty());
    assert!(repo.last_fetch.is_none());
  }

  #[test]
  fn test_registry_load_empty() {
    let temp_dir = TempDir::new().unwrap();
    let config_dirs = ConfigDirs {
      config_dir: temp_dir.path().join("config"),
      data_dir: temp_dir.path().join("data"),
      cache_dir: Some(temp_dir.path().join("cache")),
    };

    // Test loading non-existent registry
    let registry = Registry::load(&config_dirs).unwrap();
    assert!(registry.repositories.is_empty());
  }

  #[test]
  fn test_registry_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let config_dirs = ConfigDirs {
      config_dir: temp_dir.path().join("config"),
      data_dir: temp_dir.path().join("data"),
      cache_dir: Some(temp_dir.path().join("cache")),
    };

    // Create data directory
    fs::create_dir_all(&config_dirs.data_dir).unwrap();

    let mut registry = Registry {
      repositories: Vec::new(),
    };

    // Add a repository
    let repo_dir = temp_dir.path().join("test_repo");
    fs::create_dir_all(&repo_dir).unwrap();
    registry.add(&repo_dir).unwrap();

    // Save registry
    registry.save(&config_dirs).unwrap();

    // Load registry and verify
    let loaded_registry = Registry::load(&config_dirs).unwrap();
    assert_eq!(loaded_registry.repositories.len(), 1);
    assert_eq!(loaded_registry.repositories[0].name, "test_repo");
  }
}
