//! Git repository management for testing
//!
//! This module provides utilities for creating temporary git repositories
//! and changing the current working directory for testing.

use std::env;
use std::path::{Path, PathBuf};

use git2::Repository as Git2Repository;
use tempfile::TempDir;

/// A test guard that creates a temporary git repository and
/// optionally changes the current working directory to that repository.
/// The original working directory is restored when the guard is dropped.
pub struct GitRepoTestGuard {
  /// The temporary directory containing the git repository
  pub temp_dir: TempDir,
  /// The git repository
  pub repo: Git2Repository,
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
    let repo = Git2Repository::init(temp_path).expect("Failed to initialize git repository");

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
    let repo = Git2Repository::init(temp_path).expect("Failed to initialize git repository");

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
    let original_dir = env::current_dir().unwrap();

    {
      let git_repo = GitRepoTestGuard::new_and_change_dir();
      assert!(git_repo.path().join(".git").exists());

      // Current directory should be the git repo
      assert_eq!(env::current_dir().unwrap(), git_repo.path());
    }

    // After dropping, we should be back in the original directory
    assert_eq!(env::current_dir().unwrap(), original_dir);
  }

  #[test]
  fn test_change_and_restore_dir() {
    let original_dir = env::current_dir().unwrap();

    let mut git_repo = GitRepoTestGuard::new();
    assert!(git_repo.path().join(".git").exists());

    // Change directory
    git_repo.change_dir();
    assert_eq!(env::current_dir().unwrap(), git_repo.path());

    // Restore directory
    git_repo.restore_dir();
    assert_eq!(env::current_dir().unwrap(), original_dir);
  }
}
