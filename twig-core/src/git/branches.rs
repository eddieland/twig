//! Branch-related helpers built on top of repository discovery utilities.

use anyhow::{Context, Result};
use git2::{self, Repository};

use super::repository::get_repository;

/// Get the current branch name if we're in a Git repository.
pub fn current_branch() -> Result<Option<String>> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  let head = repo.head().context("Failed to get HEAD reference")?;

  if let Some(branch_name) = head.shorthand() {
    Ok(Some(branch_name.to_string()))
  } else {
    Ok(None)
  }
}

/// Check if a branch exists in the repository.
pub fn branch_exists(branch_name: &str) -> Result<bool> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(_) => Ok(true),
    Err(_) => Ok(false),
  }
}

/// Get all local branches in the repository.
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

/// Get the remote tracking branch for a local branch.
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
  use git2::Repository as GitRepository;
  use tempfile::TempDir;

  use super::*;

  fn init_repo_with_commit(temp_dir: &TempDir) -> GitRepository {
    let repo = GitRepository::init(temp_dir.path()).unwrap();

    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
      let mut index = repo.index().unwrap();
      index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    repo
      .commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
      .unwrap();

    repo
  }

  #[test]
  fn get_local_branches_lists_branches() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let repo = init_repo_with_commit(&temp_dir);

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_path).unwrap();

    let branches = get_local_branches().unwrap();
    assert!(!branches.is_empty());

    std::env::set_current_dir(original_dir).unwrap();

    drop(repo);
  }

  #[test]
  fn checkout_branch_switches_head() {
    let temp_dir = TempDir::new().unwrap();
    let repo = init_repo_with_commit(&temp_dir);

    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/test", &head_commit, false).unwrap();

    checkout_branch(&repo, "feature/test").unwrap();

    let head = repo.head().unwrap();
    assert_eq!(head.shorthand(), Some("feature/test"));
  }
}
