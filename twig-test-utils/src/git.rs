//! Git repository management for testing
//!
//! This module provides utilities for creating temporary git repositories
//! and changing the current working directory for testing.

use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::Result;
use git2::{BranchType, Repository, Signature};
use tempfile::TempDir;

/// A test guard that creates a temporary git repository and
/// optionally changes the current working directory to that repository.
/// The original working directory is restored when the guard is dropped.
pub struct GitRepoTestGuard {
  /// The temporary directory containing the git repository
  pub temp_dir: TempDir,
  /// The git repository
  pub repo: Repository,
  /// The original working directory, if changed
  original_dir: Option<PathBuf>,
}

impl GitRepoTestGuard {
  /// Create a new test git repository without changing the current working
  /// directory
  pub fn new() -> Self {
    // Create a temporary directory
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let temp_path = temp_dir.path();

    // Initialize a git repository in the temporary directory
    let repo = Repository::init(temp_path).expect("Failed to initialize git repository");

    // Set test user configuration
    let mut config = repo.config().expect("Failed to get repository config");
    config
      .set_str("user.name", "Twig Test User")
      .expect("Failed to set user.name");
    config
      .set_str("user.email", "twig-test@example.com")
      .expect("Failed to set user.email");

    // Verify that the .git directory was created
    assert!(
      temp_path.join(".git").exists(),
      "Git repository was not properly initialized"
    );

    Self {
      temp_dir,
      repo,
      original_dir: None,
    }
  }

  /// Create a new test git repository and change the current working directory
  /// to it
  pub fn new_and_change_dir() -> Self {
    // Create a temporary directory and initialize git repository
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let temp_path = temp_dir.path();

    // Initialize a git repository in the temporary directory
    let repo = Repository::init(temp_path).expect("Failed to initialize git repository");

    // Set test user configuration
    let mut config = repo.config().expect("Failed to get repository config");
    config
      .set_str("user.name", "Twig Test User")
      .expect("Failed to set user.name");
    config
      .set_str("user.email", "twig-test@example.com")
      .expect("Failed to set user.email");

    // Verify that the .git directory was created
    assert!(
      temp_path.join(".git").exists(),
      "Git repository was not properly initialized"
    );

    // Save the current directory so we can restore it later
    let original_dir = env::current_dir().expect("Failed to get current directory");

    // Change the current directory to our temporary git repository
    env::set_current_dir(temp_path).expect("Failed to change current directory");

    Self {
      temp_dir,
      repo,
      original_dir: Some(original_dir),
    }
  }

  /// Get the path to the git repository
  pub fn path(&self) -> &Path {
    self.temp_dir.path()
  }

  /// Change the current working directory to the git repository
  /// Returns the original directory so it can be restored later if needed
  pub fn change_dir(&mut self) -> PathBuf {
    // If we've already changed the directory, return early
    if self.original_dir.is_some() {
      return self.original_dir.as_ref().unwrap().clone();
    }

    // Save the current directory so we can restore it later
    let original_dir = env::current_dir().expect("Failed to get current directory");
    self.original_dir = Some(original_dir.clone());

    // Change the current directory to our temporary git repository
    env::set_current_dir(self.temp_dir.path()).expect("Failed to change current directory");

    original_dir
  }

  /// Restore the original working directory if it was changed
  pub fn restore_dir(&mut self) {
    if let Some(original_dir) = self.original_dir.take() {
      env::set_current_dir(original_dir).expect("Failed to restore original directory");
    }
  }
}

impl Default for GitRepoTestGuard {
  fn default() -> Self {
    Self::new()
  }
}

impl Drop for GitRepoTestGuard {
  fn drop(&mut self) {
    // Restore the original working directory if it was changed
    self.restore_dir();
  }
}

/// Helper function to create a commit in a repository
pub fn create_commit(repo: &Repository, file_name: &str, content: &str, message: &str) -> Result<()> {
  // Create a file
  let repo_path = repo.path().parent().unwrap();
  let file_path = repo_path.join(file_name);
  fs::write(&file_path, content)?;

  // Stage the file
  let mut index = repo.index()?;
  index.add_path(Path::new(file_name))?;
  index.write()?;

  // Create a commit
  let tree_id = index.write_tree()?;
  let tree = repo.find_tree(tree_id)?;

  let signature = Signature::now("Test User", "test@example.com")?;

  // Handle parent commits
  if let Ok(head) = repo.head() {
    if let Ok(parent) = head.peel_to_commit() {
      repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[&parent])?;
    } else {
      repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?;
    }
  } else {
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?;
  }

  Ok(())
}

/// Helper function to create a branch in a repository
pub fn create_branch(repo: &Repository, branch_name: &str, start_point: Option<&str>) -> Result<()> {
  let head = if let Some(start) = start_point {
    repo
      .find_branch(start, BranchType::Local)?
      .into_reference()
      .peel_to_commit()?
  } else {
    repo.head()?.peel_to_commit()?
  };

  repo.branch(branch_name, &head, false)?;
  Ok(())
}

/// Helper function to checkout a branch
pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
  let obj = repo
    .revparse_single(&format!("refs/heads/{branch_name}"))?
    .peel_to_commit()?;

  repo.checkout_tree(&obj.into_object(), None)?;
  repo.set_head(&format!("refs/heads/{branch_name}"))?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_new_creates_git_repo() {
    let git_repo = GitRepoTestGuard::new();
    assert!(git_repo.path().join(".git").exists());
  }

  #[test]
  fn test_new_and_change_dir() {
    let original_dir = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();

    {
      let git_repo = GitRepoTestGuard::new_and_change_dir();
      assert!(git_repo.path().join(".git").exists());

      // Current directory should be the git repo
      assert_eq!(
        std::fs::canonicalize(env::current_dir().unwrap()).unwrap(),
        std::fs::canonicalize(git_repo.path()).unwrap()
      );
    }

    // After dropping, we should be back in the original directory
    assert_eq!(
      std::fs::canonicalize(env::current_dir().unwrap()).unwrap(),
      original_dir
    );
  }

  #[test]
  fn test_change_and_restore_dir() {
    let original_dir = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();

    let mut git_repo = GitRepoTestGuard::new();
    assert!(git_repo.path().join(".git").exists());

    // Change directory
    git_repo.change_dir();
    assert_eq!(
      std::fs::canonicalize(env::current_dir().unwrap()).unwrap(),
      std::fs::canonicalize(git_repo.path()).unwrap()
    );

    // Restore directory
    git_repo.restore_dir();
    assert_eq!(
      std::fs::canonicalize(env::current_dir().unwrap()).unwrap(),
      original_dir
    );
  }
}
