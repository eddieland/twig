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
  // Resolve the branch tip to a tree object so checkout_tree can update both
  // the working directory and the index atomically, before we move HEAD.
  // (Calling set_head first and then checkout_head with a default builder
  // leaves the index stale, making files that differ between branches appear
  // as staged changes.)
  let commit = repo
    .find_branch(branch_name, git2::BranchType::Local)
    .with_context(|| format!("Branch '{branch_name}' not found"))?
    .into_reference()
    .peel_to_commit()
    .with_context(|| format!("Failed to peel branch '{branch_name}' to commit"))?;

  // Update working tree and index to match the target commit.
  repo
    .checkout_tree(
      commit.as_object(),
      Some(git2::build::CheckoutBuilder::new().safe()),
    )
    .with_context(|| format!("Failed to checkout tree for branch '{branch_name}'"))?;

  // Now update HEAD to point to the branch.
  repo
    .set_head(&format!("refs/heads/{branch_name}"))
    .with_context(|| format!("Failed to set HEAD to branch '{branch_name}'"))?;

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

    drop(tree);

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

  /// Switching branches must not leave unexpected staged changes.
  ///
  /// Regression test: the old implementation called `set_head` before
  /// `checkout_head` with a no-op `CheckoutBuilder`, so the index was never
  /// updated to match the new HEAD tree, causing files that differ between
  /// the two branches to appear staged.
  #[test]
  fn checkout_branch_leaves_no_staged_changes() {
    use std::fs;

    use git2::build::CheckoutBuilder;

    let temp_dir = TempDir::new().unwrap();
    let repo = git2::Repository::init(temp_dir.path()).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "Test").unwrap();
    cfg.set_str("user.email", "test@test.com").unwrap();
    drop(cfg);

    let signature = git2::Signature::now("Test", "test@test.com").unwrap();

    // Commit on the default branch (may be "main" or "master"): only "base.txt" exists.
    let base_file = temp_dir.path().join("base.txt");
    fs::write(&base_file, "base content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("base.txt")).unwrap();
    index.write().unwrap();
    let base_tree_oid = index.write_tree().unwrap();
    let base_tree = repo.find_tree(base_tree_oid).unwrap();
    let base_commit_oid = repo
      .commit(Some("HEAD"), &signature, &signature, "base commit", &base_tree, &[])
      .unwrap();
    let base_commit = repo.find_commit(base_commit_oid).unwrap();

    // Capture the default branch name now, while HEAD is on it.
    let default_branch_name = repo.head().unwrap().shorthand().unwrap().to_string();

    // Create a feature branch and add a file unique to it.
    repo.branch("feature", &base_commit, false).unwrap();
    let feature_obj = repo.revparse_single("refs/heads/feature").unwrap();
    repo
      .checkout_tree(&feature_obj, Some(CheckoutBuilder::new().force()))
      .unwrap();
    repo.set_head("refs/heads/feature").unwrap();

    let feature_file = temp_dir.path().join("feature-file.txt");
    fs::write(&feature_file, "feature content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("feature-file.txt")).unwrap();
    index.write().unwrap();
    let feature_tree_oid = index.write_tree().unwrap();
    let feature_tree = repo.find_tree(feature_tree_oid).unwrap();
    repo
      .commit(
        Some("HEAD"),
        &signature,
        &signature,
        "feature commit",
        &feature_tree,
        &[&base_commit],
      )
      .unwrap();

    // Switch back to the default branch so we can test switching TO feature.
    let default_obj = repo
      .revparse_single(&format!("refs/heads/{default_branch_name}"))
      .unwrap();
    repo
      .checkout_tree(&default_obj, Some(CheckoutBuilder::new().force()))
      .unwrap();
    repo
      .set_head(&format!("refs/heads/{default_branch_name}"))
      .unwrap();
    assert_eq!(
      repo.head().unwrap().shorthand(),
      Some(default_branch_name.as_str())
    );

    // Now switch to "feature" using the function under test.
    checkout_branch(&repo, "feature").unwrap();

    assert_eq!(repo.head().unwrap().shorthand(), Some("feature"));

    // The index must match HEAD — no staged (or other) changes.
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    let head_tree = head_commit.tree().unwrap();
    let diff = repo
      .diff_tree_to_index(Some(&head_tree), None, None)
      .unwrap();
    assert_eq!(
      diff.deltas().count(),
      0,
      "expected no staged changes after checkout_branch, but found {}",
      diff.deltas().count()
    );
  }
}
