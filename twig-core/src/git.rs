//! # Git Utilities
//!
//! Provides Git repository detection, branch operations, and other Git-related
//! utilities for plugins to interact with Git repositories.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::Repository;

/// Detect if the current directory or any parent directory is a Git repository
pub fn detect_repository() -> Result<Option<PathBuf>> {
  let current_dir = env::current_dir().context("Failed to get current directory")?;
  detect_repository_from_path(&current_dir)
}

/// Detect if the given path or any parent directory is a Git repository
pub fn detect_repository_from_path<P: AsRef<Path>>(path: P) -> Result<Option<PathBuf>> {
  let path = path.as_ref();

  match Repository::discover(path) {
    Ok(repo) => {
      let workdir = repo.workdir().context("Repository has no working directory")?;
      Ok(Some(workdir.to_path_buf()))
    }
    Err(_) => Ok(None),
  }
}

/// Get the current repository path if we're in a Git repository
pub fn current_repository() -> Result<Option<PathBuf>> {
  detect_repository()
}

/// Get the current branch name if we're in a Git repository
pub fn current_branch() -> Result<Option<String>> {
  let repo_path = match detect_repository()? {
    Some(path) => path,
    None => return Ok(None),
  };

  let repo = Repository::open(&repo_path).context("Failed to open Git repository")?;

  let head = repo.head().context("Failed to get HEAD reference")?;

  if let Some(branch_name) = head.shorthand() {
    Ok(Some(branch_name.to_string()))
  } else {
    Ok(None)
  }
}

/// Check if we're currently in a git repository
pub fn in_git_repository() -> bool {
  detect_repository().unwrap_or(None).is_some()
}

/// Get the Git repository object for the current directory
pub fn get_repository() -> Result<Option<Repository>> {
  let repo_path = match detect_repository()? {
    Some(path) => path,
    None => return Ok(None),
  };

  let repo = Repository::open(&repo_path).context("Failed to open Git repository")?;

  Ok(Some(repo))
}

/// Get the Git repository object for a specific path
pub fn get_repository_from_path<P: AsRef<Path>>(path: P) -> Result<Option<Repository>> {
  let repo_path = match detect_repository_from_path(path)? {
    Some(path) => path,
    None => return Ok(None),
  };

  let repo = Repository::open(&repo_path).context("Failed to open Git repository")?;

  Ok(Some(repo))
}

/// Check if a branch exists in the repository
pub fn branch_exists(branch_name: &str) -> Result<bool> {
  let repo = match get_repository()? {
    Some(repo) => repo,
    None => return Ok(false),
  };

  match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(_) => Ok(true),
    Err(_) => Ok(false),
  }
}

/// Get all local branches in the repository
pub fn get_local_branches() -> Result<Vec<String>> {
  let repo = match get_repository()? {
    Some(repo) => repo,
    None => return Ok(Vec::new()),
  };

  let branches = repo.branches(Some(git2::BranchType::Local))?;
  let mut branch_names = Vec::new();

  for branch_result in branches {
    let (branch, _) = branch_result?;
    if let Some(name) = branch.name()? {
      branch_names.push(name.to_string());
    }
  }

  Ok(branch_names)
}

/// Get the remote tracking branch for a local branch
pub fn get_upstream_branch(branch_name: &str) -> Result<Option<String>> {
  let repo = match get_repository()? {
    Some(repo) => repo,
    None => return Ok(None),
  };

  let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(branch) => branch,
    Err(_) => return Ok(None),
  };

  match branch.upstream() {
    Ok(upstream) => {
      if let Some(name) = upstream.name()? {
        Ok(Some(name.to_string()))
      } else {
        Ok(None)
      }
    }
    Err(_) => Ok(None),
  }
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_detect_repository_none() {
    let temp_dir = TempDir::new().unwrap();
    let result = detect_repository_from_path(temp_dir.path()).unwrap();
    assert!(result.is_none());
  }

  #[test]
  fn test_detect_repository_exists() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    Repository::init(repo_path).unwrap();

    let result = detect_repository_from_path(repo_path).unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), repo_path);
  }

  #[test]
  fn test_in_git_repository() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Test non-git directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(repo_path).unwrap();
    assert!(!in_git_repository());

    // Initialize git repository and test again
    Repository::init(repo_path).unwrap();
    assert!(in_git_repository());

    // Restore original directory
    env::set_current_dir(original_dir).unwrap();
  }

  #[test]
  fn test_get_local_branches() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    let repo = Repository::init(repo_path).unwrap();

    // Create an initial commit to establish main branch
    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
      let mut index = repo.index().unwrap();
      index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    repo
      .commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
      .unwrap();

    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(repo_path).unwrap();

    let branches = get_local_branches().unwrap();
    assert!(!branches.is_empty());

    // Restore original directory
    env::set_current_dir(original_dir).unwrap();
  }
}
