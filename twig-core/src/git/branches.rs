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
    .checkout_tree(commit.as_object(), Some(git2::build::CheckoutBuilder::new().safe()))
    .with_context(|| format!("Failed to checkout tree for branch '{branch_name}'"))?;

  // Now update HEAD to point to the branch.
  repo
    .set_head(&format!("refs/heads/{branch_name}"))
    .with_context(|| format!("Failed to set HEAD to branch '{branch_name}'"))?;

  Ok(())
}

/// Delete a local branch by name.
///
/// Handles a known libgit2 issue where branch deletion reports an error when
/// trying to clean up config entries that don't exist (e.g., when a branch has
/// partial config from external tools like GitHub CLI). The error looks like:
/// `"could not find key 'branch.<name>.<key>' to delete"`
///
/// When we see this specific config-cleanup error we verify that the branch
/// reference was actually removed and, if so, treat the operation as
/// successful.
///
/// See: <https://github.com/libgit2/libgit2/issues/4247>
pub fn delete_local_branch(repo: &Repository, branch_name: &str) -> Result<()> {
  let mut branch = repo
    .find_branch(branch_name, git2::BranchType::Local)
    .with_context(|| format!("Branch '{branch_name}' not found"))?;

  match branch.delete() {
    Ok(()) => Ok(()),
    Err(e) => {
      let is_config_key_error = e.class() == git2::ErrorClass::Config && e.message().contains("could not find key");

      if is_config_key_error {
        // Verify whether the branch was actually deleted despite the config error.
        if let Err(lookup_err) = repo.find_branch(branch_name, git2::BranchType::Local)
          && lookup_err.code() == git2::ErrorCode::NotFound
        {
          return Ok(());
        }
      }

      Err(e.into())
    }
  }
}

#[cfg(test)]
mod tests {
  use twig_test_utils::git::{GitRepoTestGuard, create_commit};

  use super::*;

  /// Create a test repo with a `main` branch containing `base.txt` and a
  /// `feature` branch that adds `feature.txt`.  Returns with `feature`
  /// checked out.
  fn repo_with_feature_branch() -> GitRepoTestGuard {
    let guard = GitRepoTestGuard::new();

    create_commit(&guard.repo, "base.txt", "base content\n", "initial commit").unwrap();

    {
      let main_head = guard.repo.head().unwrap().peel_to_commit().unwrap();
      guard.repo.branch("feature", &main_head, false).unwrap();
    }
    checkout_branch(&guard.repo, "feature").unwrap();
    create_commit(&guard.repo, "feature.txt", "feature work\n", "add feature file").unwrap();

    guard
  }

  #[test]
  fn get_local_branches_lists_branches() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "base.txt", "base content\n", "initial commit").unwrap();

    let branches = get_local_branches().unwrap();
    assert!(!branches.is_empty());
  }

  #[test]
  fn checkout_branch_switches_head() {
    let guard = GitRepoTestGuard::new();

    create_commit(&guard.repo, "base.txt", "base content\n", "initial commit").unwrap();

    {
      let head_commit = guard.repo.head().unwrap().peel_to_commit().unwrap();
      guard.repo.branch("feature/test", &head_commit, false).unwrap();
    }

    checkout_branch(&guard.repo, "feature/test").unwrap();

    let head = guard.repo.head().unwrap();
    assert_eq!(head.shorthand(), Some("feature/test"));
  }

  /// Regression test: switching away from a branch that has additional files
  /// must update the index and working directory so `git status` is clean on
  /// the target branch. When `checkout_branch` leaves the index stale, the
  /// source branch's files appear as phantom staged additions on the target.
  #[test]
  fn checkout_branch_updates_index_on_switch() {
    let guard = repo_with_feature_branch();
    let repo = &guard.repo;
    let repo_path = guard.path();

    // --- Operation under test ---
    checkout_branch(repo, "main").unwrap();

    // HEAD must point to main.
    assert_eq!(repo.head().unwrap().shorthand(), Some("main"));

    // The index must match main's tree — no phantom staged changes.
    let head_tree = repo.head().unwrap().peel_to_tree().unwrap();
    let index = repo.index().unwrap();
    let staged_diff = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None).unwrap();
    assert_eq!(
      staged_diff.deltas().count(),
      0,
      "Switching to main should leave no staged changes, but the index still \
       contains entries from the feature branch (stale index)."
    );

    // The working directory must match the index — no phantom unstaged changes.
    let workdir_diff = repo.diff_index_to_workdir(Some(&index), None).unwrap();
    assert_eq!(
      workdir_diff.deltas().count(),
      0,
      "Switching to main should leave no unstaged changes, but the working \
       directory still contains files from the feature branch."
    );

    // Concretely, feature.txt must not exist on main.
    assert!(
      !repo_path.join("feature.txt").exists(),
      "feature.txt should not be present in the working directory after \
       switching to main."
    );
  }

  /// Regression test: switching TO a branch whose tree has files not present
  /// on the current branch must materialise those files. When the checkout is
  /// incomplete the target branch's changes appear to be reverted—its unique
  /// files never appear and the index shows phantom staged deletions.
  #[test]
  fn checkout_branch_materializes_target_branch_files() {
    let guard = repo_with_feature_branch();
    let repo = &guard.repo;
    let repo_path = guard.path();

    // Return to main so the operation under test is the switch *to* feature.
    checkout_branch(repo, "main").unwrap();

    // Precondition: we are on main with a clean tree.
    assert!(!repo_path.join("feature.txt").exists());

    // --- Operation under test ---
    checkout_branch(repo, "feature").unwrap();

    // feature.txt must appear in the working directory.
    assert!(
      repo_path.join("feature.txt").exists(),
      "feature.txt should exist after switching to the feature branch, but \
       the file was not checked out (branch changes appear reverted)."
    );

    // The file's content must match what was committed (trim to tolerate
    // Windows \r\n line endings from core.autocrlf).
    let content = std::fs::read_to_string(repo_path.join("feature.txt")).unwrap();
    assert_eq!(
      content.trim(),
      "feature work",
      "feature.txt has unexpected content after branch switch."
    );

    // No phantom staged changes — the index must match the feature tree.
    let head_tree = repo.head().unwrap().peel_to_tree().unwrap();
    let index = repo.index().unwrap();
    let staged_diff = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None).unwrap();
    assert_eq!(
      staged_diff.deltas().count(),
      0,
      "Switching to feature should leave no staged changes, but the index \
       does not match the feature branch tree."
    );
  }

  #[test]
  fn delete_local_branch_removes_branch() {
    let guard = repo_with_feature_branch();
    let repo = &guard.repo;

    // Switch back to main so we can delete the feature branch
    checkout_branch(repo, "main").unwrap();

    assert!(repo.find_branch("feature", git2::BranchType::Local).is_ok());

    delete_local_branch(repo, "feature").unwrap();

    assert!(repo.find_branch("feature", git2::BranchType::Local).is_err());
  }

  #[test]
  fn delete_local_branch_errors_for_missing_branch() {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "base.txt", "base content\n", "initial commit").unwrap();

    let result = delete_local_branch(&guard.repo, "nonexistent");
    assert!(result.is_err());
  }
}
