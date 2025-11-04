//! # Git Utilities
//!
//! Provides Git repository detection, branch operations, and other Git-related
//! utilities for plugins to interact with Git repositories.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{self, Repository};

/// Detect if the current directory or any parent directory is a Git repository
pub fn detect_repository() -> Option<PathBuf> {
  let current_dir = env::current_dir().ok()?;
  detect_repository_from_path(&current_dir)
}

/// Detect if the given path or any parent directory is a Git repository
pub fn detect_repository_from_path<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
  let path = path.as_ref();

  match Repository::discover(path) {
    Ok(repo) => repo.workdir().map(|workdir| workdir.to_path_buf()),
    Err(_) => None,
  }
}

/// Get the current branch name if we're in a Git repository
pub fn current_branch() -> Result<Option<String>> {
  let repo_path = detect_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

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
  detect_repository().is_some()
}

/// Get the Git repository object for the current directory
pub fn get_repository() -> Option<Repository> {
  let repo_path = detect_repository()?;

  let repo = Repository::open(&repo_path)
    .context("Failed to open Git repository")
    .ok()?;

  Some(repo)
}

/// Get the Git repository object for a specific path
pub fn get_repository_from_path<P: AsRef<Path>>(path: P) -> Option<Repository> {
  let repo_path = detect_repository_from_path(path)?;

  let repo = Repository::open(&repo_path)
    .context("Failed to open Git repository")
    .ok()?;

  Some(repo)
}

/// Check if a branch exists in the repository
pub fn branch_exists(branch_name: &str) -> Result<bool> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(_) => Ok(true),
    Err(_) => Ok(false),
  }
}

/// Get all local branches in the repository
pub fn get_local_branches() -> Result<Vec<String>> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

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
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

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

/// Checkout an existing local branch using the provided repository.
pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
  let branch = repo
    .find_branch(branch_name, git2::BranchType::Local)
    .with_context(|| format!("Branch '{branch_name}' not found"))?;

  let target = branch
    .get()
    .target()
    .ok_or_else(|| anyhow::anyhow!("Branch '{branch_name}' has no target commit"))?;

  repo
    .set_head(&format!("refs/heads/{branch_name}"))
    .with_context(|| format!("Failed to set HEAD to branch '{branch_name}'"))?;

  let object = repo.find_object(target, None)?;
  let mut builder = git2::build::CheckoutBuilder::new();

  repo
    .checkout_tree(&object, Some(&mut builder))
    .with_context(|| format!("Failed to checkout branch '{branch_name}'"))?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_detect_repository_none() {
    let temp_dir = TempDir::new().unwrap();
    let result = detect_repository_from_path(temp_dir.path());
    assert!(result.is_none());
  }

  #[test]
  fn test_detect_repository_exists() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    Repository::init(repo_path).unwrap();

    let maybe_result = detect_repository_from_path(repo_path);
    assert!(maybe_result.is_some());

    let result = maybe_result.unwrap();
    assert_eq!(
      std::fs::canonicalize(result).unwrap(),
      std::fs::canonicalize(repo_path).unwrap()
    );
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

  #[test]
  fn test_checkout_branch() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let repo = Repository::init(repo_path).unwrap();

    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
      let mut index = repo.index().unwrap();
      index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    repo
      .commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
      .unwrap();

    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/test", &head_commit, false).unwrap();

    checkout_branch(&repo, "feature/test").unwrap();

    let head = repo.head().unwrap();
    assert_eq!(head.shorthand(), Some("feature/test"));
  }
}
