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

  /// Regression test: switching away from a branch that has additional files
  /// must update the index and working directory so `git status` is clean on
  /// the target branch. When `checkout_branch` leaves the index stale, the
  /// source branch's files appear as phantom staged additions on the target.
  #[test]
  fn checkout_branch_updates_index_on_switch() {
    use twig_test_utils::git::{GitRepoTestGuard, create_commit};

    let guard = GitRepoTestGuard::new();
    let repo = &guard.repo;
    let repo_path = guard.path();

    // Commit a file on main.
    create_commit(repo, "base.txt", "base content\n", "initial commit").unwrap();

    // Create a feature branch at the same commit and switch to it.
    // Both branches share the same tree here so the switch is a no-op for
    // the working directory and index—this avoids triggering the bug during
    // setup.
    let main_head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature", &main_head, false).unwrap();
    checkout_branch(repo, "feature").unwrap();

    // Add a new file on the feature branch and commit.
    create_commit(repo, "feature.txt", "feature work\n", "add feature file").unwrap();

    // --- Operation under test ---
    // Switch back to main using the production checkout_branch.
    checkout_branch(repo, "main").unwrap();

    // HEAD must point to main.
    assert_eq!(repo.head().unwrap().shorthand(), Some("main"));

    // The index must match main's tree — no phantom staged changes.
    let head_tree = repo.head().unwrap().peel_to_tree().unwrap();
    let index = repo.index().unwrap();
    let staged_diff = repo
      .diff_tree_to_index(Some(&head_tree), Some(&index), None)
      .unwrap();
    assert_eq!(
      staged_diff.deltas().count(),
      0,
      "Switching to main should leave no staged changes, but the index still \
       contains entries from the feature branch (stale index)."
    );

    // The working directory must match the index — no phantom unstaged changes.
    let workdir_diff = repo
      .diff_index_to_workdir(Some(&index), None)
      .unwrap();
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
    use twig_test_utils::git::{self, GitRepoTestGuard, create_commit};

    let guard = GitRepoTestGuard::new();
    let repo = &guard.repo;
    let repo_path = guard.path();

    // Commit a file on main.
    create_commit(repo, "base.txt", "base content\n", "initial commit").unwrap();

    // Build the feature branch using the (correct) test-utils checkout so
    // that the setup itself is not affected by a production checkout bug.
    let main_head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature", &main_head, false).unwrap();
    git::checkout_branch(repo, "feature").unwrap();
    create_commit(repo, "feature.txt", "feature work\n", "add feature file").unwrap();

    // Return to main (again using the correct test-utils checkout).
    git::checkout_branch(repo, "main").unwrap();

    // Precondition: we are on main with a clean tree.
    assert!(!repo_path.join("feature.txt").exists());

    // --- Operation under test ---
    // Switch to feature using the production checkout_branch.
    checkout_branch(repo, "feature").unwrap();

    // feature.txt must appear in the working directory.
    assert!(
      repo_path.join("feature.txt").exists(),
      "feature.txt should exist after switching to the feature branch, but \
       the file was not checked out (branch changes appear reverted)."
    );

    // The file's content must match what was committed.
    let content = std::fs::read_to_string(repo_path.join("feature.txt")).unwrap();
    assert_eq!(
      content, "feature work\n",
      "feature.txt has unexpected content after branch switch."
    );

    // No phantom staged changes — the index must match the feature tree.
    let head_tree = repo.head().unwrap().peel_to_tree().unwrap();
    let index = repo.index().unwrap();
    let staged_diff = repo
      .diff_tree_to_index(Some(&head_tree), Some(&index), None)
      .unwrap();
    assert_eq!(
      staged_diff.deltas().count(),
      0,
      "Switching to feature should leave no staged changes, but the index \
       does not match the feature branch tree."
    );
  }
}
