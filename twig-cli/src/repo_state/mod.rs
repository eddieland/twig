//! # Repository State Management
//!
//! Manages persistent state for Git repositories, including branch
//! dependencies, metadata, and configuration storage for the twig tool.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use git2::Repository as Git2Repository;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
  pub config_overrides: serde_json::Value,

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
    // Use the ConfigDirs to get the state path
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
        config_overrides: serde_json::Value::Object(serde_json::Map::new()),
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

  /// Rebuild all indices for fast lookups
  fn rebuild_indices(&mut self) {
    // Clear existing indices
    self.branch_to_jira_index.clear();
    self.jira_to_branch_index.clear();
    self.dependency_children_index.clear();
    self.dependency_parents_index.clear();

    // Build Jira indices
    for (branch_name, metadata) in &self.branches {
      if let Some(jira_key) = &metadata.jira_issue {
        self.branch_to_jira_index.insert(branch_name.clone(), jira_key.clone());
        self.jira_to_branch_index.insert(jira_key.clone(), branch_name.clone());
      }
    }

    // Build dependency indices
    for dep in &self.dependencies {
      // Parent -> children mapping
      self
        .dependency_children_index
        .entry(dep.parent.clone())
        .or_default()
        .push(dep.child.clone());

      // Child -> parents mapping
      self
        .dependency_parents_index
        .entry(dep.child.clone())
        .or_default()
        .push(dep.parent.clone());
    }
  }

  /// Save the repository state to disk
  pub fn save<P: AsRef<Path>>(&self, repo_path: P) -> Result<()> {
    // Use the ConfigDirs to get the state directory and path
    let config_dirs = crate::config::ConfigDirs::new()?;
    let twig_dir = config_dirs.repo_state_dir(&repo_path);

    if !twig_dir.exists() {
      fs::create_dir_all(&twig_dir).context("Failed to create .twig directory")?;

      // Add .twig to .gitignore if it doesn't already contain it
      let gitignore_path = repo_path.as_ref().join(".gitignore");
      let mut gitignore_content = String::new();
      let mut needs_twig_entry = true;

      if gitignore_path.exists() {
        gitignore_content = fs::read_to_string(&gitignore_path).context("Failed to read .gitignore file")?;

        // Check if .twig is already in .gitignore
        if gitignore_content.lines().any(|line| line.trim() == ".twig/") {
          needs_twig_entry = false;
        }
      }

      if needs_twig_entry {
        // Ensure there's a newline before adding .twig/
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
          gitignore_content.push('\n');
        }

        gitignore_content.push_str(".twig/\n");
        fs::write(&gitignore_path, gitignore_content).context("Failed to update .gitignore file")?;
      }
    }

    // Update timestamp before saving
    let mut state_to_save = self.clone();
    state_to_save.updated_at = Utc::now();

    let state_path = config_dirs.repo_state_path(&repo_path);
    let content = serde_json::to_string_pretty(&state_to_save).context("Failed to serialize state")?;

    fs::write(&state_path, content).context("Failed to write state file")?;

    Ok(())
  }

  /// Add a worktree to the state
  pub fn add_worktree(&mut self, worktree: Worktree) {
    // Remove any existing worktree with the same name
    self.worktrees.retain(|w| w.name != worktree.name);
    self.worktrees.push(worktree);
  }

  /// Remove a worktree from the state
  pub fn remove_worktree(&mut self, name: &str) -> bool {
    let initial_len = self.worktrees.len();
    self.worktrees.retain(|w| w.name != name);
    self.worktrees.len() < initial_len
  }

  /// Get a worktree by name
  pub fn get_worktree(&self, name: &str) -> Option<&Worktree> {
    self.worktrees.iter().find(|w| w.name == name)
  }

  /// List all worktrees
  pub fn list_worktrees(&self) -> &[Worktree] {
    &self.worktrees
  }

  /// Add a branch-issue association
  pub fn add_branch_issue(&mut self, branch_issue: BranchMetadata) {
    let branch_name = branch_issue.branch.clone();
    self.branches.insert(branch_name, branch_issue);
    self.rebuild_indices();
  }

  /// Get a branch-issue association by branch name
  pub fn get_branch_issue_by_branch(&self, branch: &str) -> Option<&BranchMetadata> {
    self.branches.get(branch)
  }

  /// Get a branch-issue association by Jira issue key
  #[allow(dead_code)]
  pub fn get_branch_issue_by_jira(&self, jira_issue: &str) -> Option<&BranchMetadata> {
    // Use the pre-built index for O(1) lookup
    if let Some(branch_name) = self.jira_to_branch_index.get(jira_issue) {
      self.branches.get(branch_name)
    } else {
      None
    }
  }

  /// List all branch-issue associations
  #[allow(dead_code)]
  pub fn list_branch_issues(&self) -> Vec<&BranchMetadata> {
    self.branches.values().collect()
  }

  // === Dependency Management Methods ===

  /// Add a user-defined branch dependency
  pub fn add_dependency(&mut self, child: String, parent: String) -> Result<()> {
    // Check if the dependency already exists
    if self.dependencies.iter().any(|d| d.child == child && d.parent == parent) {
      return Err(anyhow::anyhow!(
        "Dependency from '{}' to '{}' already exists",
        child,
        parent
      ));
    }

    // Check for circular dependencies
    if self.would_create_cycle(&child, &parent)? {
      return Err(anyhow::anyhow!(
        "Adding dependency from '{}' to '{}' would create a circular dependency",
        child,
        parent
      ));
    }

    // Create the new dependency
    let dependency = BranchDependency {
      id: Uuid::new_v4(),
      child: child.clone(),
      parent: parent.clone(),
      created_at: Utc::now(),
    };

    self.dependencies.push(dependency);
    self.rebuild_indices();
    Ok(())
  }

  /// Remove a user-defined branch dependency
  pub fn remove_dependency(&mut self, child: &str, parent: &str) -> bool {
    let initial_len = self.dependencies.len();
    self.dependencies.retain(|d| !(d.child == child && d.parent == parent));
    let removed = self.dependencies.len() < initial_len;

    if removed {
      self.rebuild_indices();
    }

    removed
  }

  /// Remove all dependencies for a branch (both as child and parent)
  #[allow(dead_code)]
  pub fn remove_all_dependencies_for_branch(&mut self, branch: &str) -> usize {
    let initial_len = self.dependencies.len();
    self.dependencies.retain(|d| d.child != branch && d.parent != branch);
    let removed_count = initial_len - self.dependencies.len();

    if removed_count > 0 {
      self.rebuild_indices();
    }

    removed_count
  }

  /// Get all children of a branch (branches that depend on this branch)
  #[allow(dead_code)]
  pub fn get_dependency_children(&self, parent: &str) -> Vec<&str> {
    self
      .dependency_children_index
      .get(parent)
      .map(|children| children.iter().map(|s| s.as_str()).collect())
      .unwrap_or_default()
  }

  /// Get all parents of a branch (branches this branch depends on)
  #[allow(dead_code)]
  pub fn get_dependency_parents(&self, child: &str) -> Vec<&str> {
    self
      .dependency_parents_index
      .get(child)
      .map(|parents| parents.iter().map(|s| s.as_str()).collect())
      .unwrap_or_default()
  }

  /// Check if adding a dependency would create a cycle
  fn would_create_cycle(&self, child: &str, parent: &str) -> Result<bool> {
    // If parent depends on child (directly or indirectly), adding child->parent
    // would create a cycle
    self.has_dependency_path(parent, child)
  }

  /// Check if there's a dependency path from start to end
  fn has_dependency_path(&self, start: &str, end: &str) -> Result<bool> {
    use std::collections::{HashSet, VecDeque};

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
      if current == end {
        return Ok(true);
      }

      // Get all parents of the current branch (what this branch depends on)
      if let Some(parents) = self.dependency_parents_index.get(current) {
        for parent in parents {
          if !visited.contains(parent.as_str()) {
            visited.insert(parent.as_str());
            queue.push_back(parent.as_str());
          }
        }
      }
    }

    Ok(false)
  }

  /// List all dependencies
  #[allow(dead_code)]
  pub fn list_dependencies(&self) -> &[BranchDependency] {
    &self.dependencies
  }

  // === Root Branch Management Methods ===

  /// Add a root branch
  pub fn add_root(&mut self, branch: String, is_default: bool) -> Result<()> {
    // Check if the branch is already a root
    let existing_index = self.root_branches.iter().position(|r| r.branch == branch);

    if let Some(index) = existing_index {
      // Branch is already a root, just update the default flag if needed
      if is_default {
        // Remove default from all roots first
        for root in &mut self.root_branches {
          root.is_default = false;
        }
        // Set this one as default
        self.root_branches[index].is_default = true;
      }
      return Ok(());
    }

    // If this is set as default, remove default from all other roots
    if is_default {
      for root in &mut self.root_branches {
        root.is_default = false;
      }
    }

    // Create the new root branch
    let root_branch = RootBranch {
      id: Uuid::new_v4(),
      branch: branch.clone(),
      is_default,
      created_at: Utc::now(),
    };

    self.root_branches.push(root_branch);
    Ok(())
  }

  /// Remove a root branch
  pub fn remove_root(&mut self, branch: &str) -> bool {
    let initial_len = self.root_branches.len();
    self.root_branches.retain(|r| r.branch != branch);
    self.root_branches.len() < initial_len
  }

  /// Set a root branch as the default
  #[allow(dead_code)]
  pub fn set_default_root(&mut self, branch: &str) -> Result<()> {
    // Find the root branch
    let mut found = false;
    for root in &mut self.root_branches {
      if root.branch == branch {
        root.is_default = true;
        found = true;
      } else {
        root.is_default = false;
      }
    }

    if !found {
      return Err(anyhow::anyhow!("Branch '{}' is not marked as a root", branch));
    }

    Ok(())
  }

  /// Get the default root branch
  pub fn get_default_root(&self) -> Option<&str> {
    self
      .root_branches
      .iter()
      .find(|r| r.is_default)
      .map(|r| r.branch.as_str())
  }

  /// List all root branches
  pub fn list_roots(&self) -> &[RootBranch] {
    &self.root_branches
  }

  /// Check if a branch is marked as a root
  #[allow(dead_code)]
  pub fn is_root(&self, branch: &str) -> bool {
    self.root_branches.iter().any(|r| r.branch == branch)
  }

  /// Check if there are any user-defined dependencies
  pub fn has_user_defined_dependencies(&self) -> bool {
    !self.dependencies.is_empty()
  }

  /// Get all root branch names
  pub fn get_root_branches(&self) -> Vec<String> {
    self.root_branches.iter().map(|r| r.branch.clone()).collect()
  }
}

/// Create a new worktree
pub fn create_worktree<P: AsRef<Path>>(repo_path: P, branch_name: &str) -> Result<PathBuf> {
  use crate::utils::output::{format_repo_path, print_info, print_success, print_warning};

  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Determine the worktree path
  // By default, create worktrees in a directory named after the repo with
  // "-worktrees" suffix
  let repo_name = repo_path.file_name().and_then(|n| n.to_str()).unwrap_or("repo");

  let parent_dir = repo_path.parent().unwrap_or(Path::new("."));
  let worktrees_dir = parent_dir.join(format!("{repo_name}-worktrees"));

  // Create the worktrees directory if it doesn't exist
  if !worktrees_dir.exists() {
    fs::create_dir_all(&worktrees_dir).context(format!(
      "Failed to create worktrees directory at {}",
      worktrees_dir.display()
    ))?;
  }

  // Sanitize branch name for use as directory name
  let safe_branch_name = branch_name.replace('/', "-");
  let worktree_path = worktrees_dir.join(&safe_branch_name);

  print_info(&format!(
    "Creating worktree at {}",
    format_repo_path(&worktree_path.display().to_string())
  ));

  // Check if branch exists
  let branch_exists = repo.find_branch(branch_name, git2::BranchType::Local).is_ok();

  // Also check if a branch with the sanitized name exists (could happen with
  // previous attempts)
  let sanitized_branch_exists = repo.find_branch(&safe_branch_name, git2::BranchType::Local).is_ok();

  if branch_exists {
    // Use existing branch
    print_info(&format!("Using existing branch: {branch_name}"));
    // Check if the worktree directory already exists
    if worktree_path.exists() {
      print_warning(&format!(
        "Worktree directory already exists at {}",
        format_repo_path(&worktree_path.display().to_string())
      ));
      return Err(anyhow::anyhow!(
        "Worktree directory already exists at {}. Please remove it or use a different branch name.",
        worktree_path.display()
      ));
    }

    // Check if a worktree with this name already exists
    if repo.find_worktree(&safe_branch_name).is_ok() {
      print_warning(&format!("A worktree named '{safe_branch_name}' already exists",));
      return Err(anyhow::anyhow!(
        "A worktree named '{}' already exists. This could be due to a previous attempt to create this worktree.",
        safe_branch_name
      ));
    }

    // Check if a branch with the sanitized name exists (would conflict with
    // worktree creation)
    if sanitized_branch_exists {
      print_warning(&format!(
        "A branch named '{safe_branch_name}' already exists, which conflicts with the worktree name",
      ));
      return Err(anyhow::anyhow!(
        "A branch named '{}' already exists, which conflicts with the worktree name. Please delete this branch first or use a different branch name.",
        safe_branch_name
      ));
    }

    // Try to create the worktree
    match repo.worktree(safe_branch_name.as_str(), worktree_path.as_path(), None) {
      Ok(worktree) => worktree,
      Err(err) => {
        // Get the raw error message from git2
        let git_error = err.message();

        return Err(anyhow::anyhow!(
          "Failed to create worktree for branch '{}': {}. This could be due to:
  - The worktree directory already exists but is not registered with Git
  - The branch is already checked out in another worktree
  - There are uncommitted changes that conflict with the branch
  - You don't have permission to create directories at {}",
          branch_name,
          git_error,
          worktree_path.parent().unwrap_or(Path::new(".")).display()
        ));
      }
    };
  } else {
    // Create a new branch
    print_info(&format!("Creating new branch: {branch_name}"));

    // Get the HEAD commit to branch from
    let head = repo.head()?;
    let target = head
      .target()
      .ok_or_else(|| anyhow::anyhow!("HEAD is not a direct reference"))?;
    let commit = repo.find_commit(target)?;

    // Create the branch
    repo
      .branch(branch_name, &commit, false)
      .context(format!("Failed to create branch '{branch_name}'"))?;

    // Create the worktree
    // Check if the worktree directory already exists
    if worktree_path.exists() {
      print_warning(&format!(
        "Worktree directory already exists at {}",
        format_repo_path(&worktree_path.display().to_string())
      ));
      return Err(anyhow::anyhow!(
        "Worktree directory already exists at {}. Please remove it or use a different branch name.",
        worktree_path.display()
      ));
    }

    // Check if a worktree with this name already exists
    if repo.find_worktree(&safe_branch_name).is_ok() {
      print_warning(&format!("A worktree named '{safe_branch_name}' already exists",));
      return Err(anyhow::anyhow!(
        "A worktree named '{safe_branch_name}' already exists. This could be due to a previous attempt to create this worktree.",
      ));
    }

    // Check if a branch with the sanitized name exists (would conflict with
    // worktree creation)
    if repo.find_branch(&safe_branch_name, git2::BranchType::Local).is_ok() {
      print_warning(&format!(
        "A branch named '{safe_branch_name}' already exists, which conflicts with the worktree name",
      ));
      return Err(anyhow::anyhow!(
        "A branch named '{safe_branch_name}' already exists, which conflicts with the worktree name. Please delete this branch first or use a different branch name.",
      ));
    }

    // Try to create the worktree
    match repo.worktree(safe_branch_name.as_str(), worktree_path.as_path(), None) {
      Ok(worktree) => worktree,
      Err(err) => {
        // Get the raw error message from git2
        let git_error = err.message();

        return Err(anyhow::anyhow!(
          "Failed to create worktree for branch '{}': {}. This could be due to:
  - The worktree directory already exists but is not registered with Git
  - The branch is already checked out in another worktree
  - There are uncommitted changes that conflict with the branch
  - You don't have permission to create directories at {}",
          branch_name,
          git_error,
          worktree_path.parent().unwrap_or(Path::new(".")).display()
        ));
      }
    };
  }

  // Update the repository state
  let mut state = RepoState::load(repo_path)?;

  // Get current timestamp
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
    .unwrap()
    .to_rfc3339();

  // Add the worktree to the state
  state.add_worktree(Worktree {
    name: safe_branch_name,
    path: worktree_path.to_string_lossy().to_string(),
    branch: branch_name.to_string(),
    created_at: time_str,
  });

  state.save(repo_path)?;

  print_success(&format!(
    "Successfully created worktree for branch '{}' at {}",
    branch_name,
    format_repo_path(&worktree_path.display().to_string())
  ));

  Ok(worktree_path)
}

/// List all worktrees for a repository
pub fn list_worktrees<P: AsRef<Path>>(repo_path: P) -> Result<()> {
  use crate::utils::output::{
    format_command, format_repo_path, format_timestamp, print_header, print_info, print_warning,
  };

  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the list of worktrees from git
  let worktree_names = repo.worktrees()?;

  if worktree_names.is_empty() {
    print_warning("No worktrees found for this repository.");
    print_info(&format!(
      "Create one with {}",
      format_command("twig worktree create <branch-name>")
    ));
    return Ok(());
  }

  // Load the repository state to get additional metadata
  let state = RepoState::load(repo_path)?;

  print_header("Worktrees");

  // Get all worktrees from the state
  let state_worktrees = state.list_worktrees();

  // Iterate through the worktree names
  for i in 0..worktree_names.len() {
    if let Some(name) = worktree_names.get(i) {
      let worktree = repo.find_worktree(name)?;
      let path = worktree.path().to_string_lossy().to_string();

      // Try to get additional metadata from the state
      let state_worktree = state.get_worktree(name);

      println!("  Branch: {name}",);
      println!("  Path: {}", format_repo_path(&path));

      if let Some(wt) = state_worktree {
        println!("  Created: {}", format_timestamp(&wt.created_at));
      } else {
        // If we don't have metadata in the state, check if we have any worktrees in the
        // state
        if !state_worktrees.is_empty() {
          println!("  Created: Unknown (no metadata available)");
        }
      }

      println!();
    }
  }

  Ok(())
}

/// Clean up stale worktrees
pub fn clean_worktrees<P: AsRef<Path>>(repo_path: P) -> Result<()> {
  use crate::utils::output::{print_info, print_success, print_warning};

  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the list of worktrees from git
  let worktree_names = repo.worktrees()?;

  if worktree_names.is_empty() {
    print_warning("No worktrees found for this repository.");
    return Ok(());
  }

  // Load the repository state
  let mut state = RepoState::load(repo_path)?;
  let mut cleaned_count = 0;

  // Iterate through the worktree names
  for i in 0..worktree_names.len() {
    if let Some(name) = worktree_names.get(i) {
      let worktree = repo.find_worktree(name)?;
      let path = worktree.path();

      // Check if the worktree directory still exists
      if !path.exists() {
        print_info(&format!(
          "Cleaning up stale worktree reference: {name} (path no longer exists)",
        ));

        // Prune the worktree reference
        worktree.prune(None)?;

        // Remove from state
        state.remove_worktree(name);

        cleaned_count += 1;
      }
    }
  }

  // Save the updated state
  state.save(repo_path)?;

  if cleaned_count > 0 {
    print_success(&format!("Cleaned up {cleaned_count} stale worktree references"));
  } else {
    print_info("No stale worktrees found to clean up");
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::fs;

  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_repo_state_creation() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let state = RepoState::load(repo_path).unwrap();

    assert_eq!(state.version, 1);
    assert!(state.worktrees.is_empty());
    assert!(state.branches.is_empty());
    assert!(state.dependencies.is_empty());
    assert!(state.root_branches.is_empty());
  }

  #[test]
  fn test_add_worktree() {
    let mut state = RepoState::default();

    let worktree = Worktree {
      name: "test-worktree".to_string(),
      path: "/path/to/worktree".to_string(),
      branch: "feature-branch".to_string(),
      created_at: "2023-01-01T00:00:00Z".to_string(),
    };

    state.add_worktree(worktree);
    assert_eq!(state.worktrees.len(), 1);
    assert_eq!(state.worktrees[0].name, "test-worktree");
  }

  #[test]
  fn test_add_duplicate_worktree() {
    let mut state = RepoState::default();

    let worktree1 = Worktree {
      name: "test-worktree".to_string(),
      path: "/path/to/worktree1".to_string(),
      branch: "branch1".to_string(),
      created_at: "2023-01-01T00:00:00Z".to_string(),
    };

    let worktree2 = Worktree {
      name: "test-worktree".to_string(),
      path: "/path/to/worktree2".to_string(),
      branch: "branch2".to_string(),
      created_at: "2023-01-02T00:00:00Z".to_string(),
    };

    state.add_worktree(worktree1);
    state.add_worktree(worktree2);

    // Should only have one worktree (the newer one)
    assert_eq!(state.worktrees.len(), 1);
    assert_eq!(state.worktrees[0].path, "/path/to/worktree2");
  }

  #[test]
  fn test_remove_worktree() {
    let mut state = RepoState::default();

    let worktree = Worktree {
      name: "test-worktree".to_string(),
      path: "/path/to/worktree".to_string(),
      branch: "feature-branch".to_string(),
      created_at: "2023-01-01T00:00:00Z".to_string(),
    };

    state.add_worktree(worktree);
    assert_eq!(state.worktrees.len(), 1);

    let removed = state.remove_worktree("test-worktree");
    assert!(removed);
    assert_eq!(state.worktrees.len(), 0);

    // Try removing non-existent worktree
    let removed = state.remove_worktree("nonexistent");
    assert!(!removed);
  }

  #[test]
  fn test_set_branch_jira_issue() {
    let mut state = RepoState::default();

    let metadata = BranchMetadata {
      branch: "feature-branch".to_string(),
      jira_issue: Some("PROJ-123".to_string()),
      github_pr: None,
      created_at: chrono::Utc::now().to_rfc3339(),
    };
    state.add_branch_issue(metadata);

    assert_eq!(state.branches.len(), 1);
    assert!(state.branches.contains_key("feature-branch"));
    assert_eq!(
      state.branches["feature-branch"].jira_issue,
      Some("PROJ-123".to_string())
    );

    // Check indices were built
    assert_eq!(
      state.branch_to_jira_index.get("feature-branch"),
      Some(&"PROJ-123".to_string())
    );
    assert_eq!(
      state.jira_to_branch_index.get("PROJ-123"),
      Some(&"feature-branch".to_string())
    );
  }

  #[test]
  fn test_set_branch_github_pr() {
    let mut state = RepoState::default();

    let metadata = BranchMetadata {
      branch: "feature-branch".to_string(),
      jira_issue: None,
      github_pr: Some(123),
      created_at: chrono::Utc::now().to_rfc3339(),
    };
    state.add_branch_issue(metadata);

    assert_eq!(state.branches.len(), 1);
    assert!(state.branches.contains_key("feature-branch"));
    assert_eq!(state.branches["feature-branch"].github_pr, Some(123));
  }

  #[test]
  fn test_add_dependency() {
    let mut state = RepoState::default();

    state
      .add_dependency("child-branch".to_string(), "parent-branch".to_string())
      .unwrap();

    assert_eq!(state.dependencies.len(), 1);
    assert_eq!(state.dependencies[0].child, "child-branch");
    assert_eq!(state.dependencies[0].parent, "parent-branch");

    // Check indices were updated
    assert_eq!(state.get_dependency_children("parent-branch"), vec!["child-branch"]);
    assert_eq!(state.get_dependency_parents("child-branch"), vec!["parent-branch"]);
  }

  #[test]
  fn test_remove_dependency() {
    let mut state = RepoState::default();

    state
      .add_dependency("child-branch".to_string(), "parent-branch".to_string())
      .unwrap();
    assert_eq!(state.dependencies.len(), 1);

    let removed = state.remove_dependency("child-branch", "parent-branch");
    assert!(removed);
    assert_eq!(state.dependencies.len(), 0);

    // Check indices were updated
    assert!(state.get_dependency_children("parent-branch").is_empty());
    assert!(state.get_dependency_parents("child-branch").is_empty());
  }

  #[test]
  fn test_add_root_branch() {
    let mut state = RepoState::default();

    state.add_root("main".to_string(), true).unwrap();

    assert_eq!(state.root_branches.len(), 1);
    assert_eq!(state.root_branches[0].branch, "main");
    assert!(state.root_branches[0].is_default);
  }

  #[test]
  fn test_multiple_root_branches_only_one_default() {
    let mut state = RepoState::default();

    state.add_root("main".to_string(), true).unwrap();
    state.add_root("develop".to_string(), true).unwrap(); // This should make main non-default

    assert_eq!(state.root_branches.len(), 2);

    let default_count = state.root_branches.iter().filter(|r| r.is_default).count();
    assert_eq!(default_count, 1);

    let default_branch = state.root_branches.iter().find(|r| r.is_default).unwrap();
    assert_eq!(default_branch.branch, "develop");
  }

  #[test]
  fn test_remove_root_branch() {
    let mut state = RepoState::default();

    state.add_root("main".to_string(), true).unwrap();
    assert_eq!(state.root_branches.len(), 1);

    let removed = state.remove_root("main");
    assert!(removed);
    assert_eq!(state.root_branches.len(), 0);
  }

  #[test]
  fn test_rebuild_indices() {
    let mut state = RepoState::default();

    // Add some data using existing methods
    let metadata1 = BranchMetadata {
      branch: "feature-1".to_string(),
      jira_issue: Some("PROJ-123".to_string()),
      github_pr: None,
      created_at: chrono::Utc::now().to_rfc3339(),
    };
    let metadata2 = BranchMetadata {
      branch: "feature-2".to_string(),
      jira_issue: Some("PROJ-456".to_string()),
      github_pr: None,
      created_at: chrono::Utc::now().to_rfc3339(),
    };
    state.add_branch_issue(metadata1);
    state.add_branch_issue(metadata2);
    state
      .add_dependency("feature-1".to_string(), "main".to_string())
      .unwrap();
    state
      .add_dependency("feature-2".to_string(), "main".to_string())
      .unwrap();

    // Clear indices manually to test rebuilding
    state.branch_to_jira_index.clear();
    state.jira_to_branch_index.clear();
    state.dependency_children_index.clear();
    state.dependency_parents_index.clear();

    // Rebuild indices
    state.rebuild_indices();

    // Verify indices were rebuilt correctly
    assert_eq!(state.branch_to_jira_index.len(), 2);
    assert_eq!(state.jira_to_branch_index.len(), 2);
    assert_eq!(state.dependency_children_index["main"].len(), 2);
    assert_eq!(state.dependency_parents_index["feature-1"].len(), 1);
    assert_eq!(state.dependency_parents_index["feature-2"].len(), 1);
  }

  #[test]
  fn test_save_and_load_state() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create initial state
    let mut state = RepoState::default();
    state.add_worktree(Worktree {
      name: "test".to_string(),
      path: "/test/path".to_string(),
      branch: "feature".to_string(),
      created_at: "2023-01-01T00:00:00Z".to_string(),
    });

    let metadata = BranchMetadata {
      branch: "feature".to_string(),
      jira_issue: Some("PROJ-123".to_string()),
      github_pr: None,
      created_at: chrono::Utc::now().to_rfc3339(),
    };
    state.add_branch_issue(metadata);
    state.add_dependency("feature".to_string(), "main".to_string()).unwrap();

    // Save state
    state.save(repo_path).unwrap();

    // Load state and verify
    let loaded_state = RepoState::load(repo_path).unwrap();
    assert_eq!(loaded_state.worktrees.len(), 1);
    assert_eq!(loaded_state.branches.len(), 1);
    assert_eq!(loaded_state.dependencies.len(), 1);
    assert_eq!(loaded_state.worktrees[0].name, "test");
    assert_eq!(
      loaded_state.branches["feature"].jira_issue,
      Some("PROJ-123".to_string())
    );
  }

  #[test]
  fn test_gitignore_creation() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let state = RepoState::default();
    state.save(repo_path).unwrap();

    // Check that .gitignore was created with .twig/ entry
    let gitignore_path = repo_path.join(".gitignore");
    assert!(gitignore_path.exists());

    let gitignore_content = fs::read_to_string(gitignore_path).unwrap();
    assert!(gitignore_content.contains(".twig/"));
  }

  #[test]
  fn test_gitignore_append() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create existing .gitignore
    let gitignore_path = repo_path.join(".gitignore");
    fs::write(&gitignore_path, "*.log\ntarget/").unwrap();

    let state = RepoState::default();
    state.save(repo_path).unwrap();

    // Check that .twig/ was appended
    let gitignore_content = fs::read_to_string(gitignore_path).unwrap();
    assert!(gitignore_content.contains("*.log"));
    assert!(gitignore_content.contains("target/"));
    assert!(gitignore_content.contains(".twig/"));
  }
}
