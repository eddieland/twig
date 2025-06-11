//! # Utility Functions
//!
//! Common utility functions and helpers for file operations, Git repository
//! validation, and shared functionality across the twig application.

use std::path::PathBuf;

use anyhow::{Context, Result};
use twig_core::{detect_repository, detect_repository_from_path};

/// Resolve a repository path from a command line argument or current directory
pub fn resolve_repository_path(repo_arg: Option<&str>) -> Result<PathBuf> {
  match repo_arg {
    Some(path) => {
      let path_buf = PathBuf::from(path);
      if !path_buf.exists() {
        return Err(anyhow::anyhow!("Repository path does not exist: {}", path));
      }
      detect_repository_from_path(&path_buf).context(format!("Failed to detect repository at path: {path}"))
    }
    None => {
      // Try to detect the current repository
      detect_repository().context("No repository specified and not in a git repository")
    }
  }
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;
  use twig_test_utils::GitRepoTestGuard;

  use super::*;

  // Test resolve_repository_path with a valid path
  #[test]
  fn test_resolve_repository_path_with_valid_path() {
    // Create a temporary directory to use as our "repository"
    let temp_dir = TempDir::new().unwrap();

    // This is a bit of a hack, but we can't easily mock these functions
    // without changing the code structure, so we'll just test the error path
    let result = resolve_repository_path(Some(temp_dir.path().to_str().unwrap()));

    // If the path exists but isn't a git repo, we'll get an error about failing to
    // detect repository
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to detect repository"));
  }

  // Test resolve_repository_path with an invalid path
  #[test]
  fn test_resolve_repository_path_with_invalid_path() {
    let result = resolve_repository_path(Some("/path/that/does/not/exist"));
    assert!(result.is_err());
    assert!(
      result
        .unwrap_err()
        .to_string()
        .contains("Repository path does not exist")
    );
  }

  // Test resolve_repository_path with None (current directory)
  #[test]
  fn test_resolve_repository_path_with_none() {
    // Create a temporary git repository and change to its directory
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let git_repo_path = std::fs::canonicalize(git_repo.path()).unwrap();

    // Now test the function with None
    let result = resolve_repository_path(None);

    // The result should be Ok and contain our temporary directory path
    assert!(result.is_ok());
    let repo_path = std::fs::canonicalize(result.unwrap()).unwrap();
    assert_eq!(repo_path, git_repo_path);
  }
}
