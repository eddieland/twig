//! # Fixup Commit Creation
//!
//! This module handles the creation of fixup commits that target existing
//! commits in the Git history. It provides functionality to create properly
//! formatted fixup commits that can be automatically squashed during
//! interactive rebase.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use git2::Repository;
use twig_core::output::{print_error, print_warning};

use crate::consts;
use crate::fixup::commit_collector::CommitCandidate;

/// Create a fixup commit for the selected target commit
pub fn create_fixup_commit(repo_path: &Path, target_commit: &CommitCandidate) -> Result<()> {
  // Check if there are staged changes
  if !has_staged_changes(repo_path)? {
    print_warning("No staged changes found. Stage changes first before creating a fixup commit.");
    return Ok(());
  }

  tracing::debug!("Creating fixup commit for {}", target_commit.hash);

  // Create the fixup commit
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(["commit", "--fixup", &target_commit.hash])
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git commit --fixup command")?;

  if output.status.success() {
    tracing::debug!("Fixup commit created successfully");
    tracing::trace!("Git output: {}", String::from_utf8_lossy(&output.stdout));
    Ok(())
  } else {
    let stderr = String::from_utf8_lossy(&output.stderr);
    print_error("Failed to create fixup commit.");
    tracing::warn!("Git commit --fixup failed: {}", stderr);
    Err(anyhow::anyhow!("Git commit --fixup command failed: {stderr}"))
  }
}

/// Checks if there are staged changes in the Git repository.
///
/// This function uses git2 to determine if there are any staged changes
/// ready to be committed. This is essential before creating a fixup commit,
/// as Git requires changes to commit.
///
/// # Arguments
///
/// * `repo_path` - Path to the Git repository root
///
/// # Returns
///
/// Returns `true` if there are staged changes, `false` if the staging area is
/// clean.
///
/// # Errors
///
/// Returns an error if the repository path is invalid or git2 operations fail.
pub fn has_staged_changes(repo_path: &Path) -> Result<bool> {
  let repo = Repository::open(repo_path).context("Failed to open git repository")?;

  // Get the index (staging area)
  let index = repo.index().context("Failed to get repository index")?;

  // Get HEAD tree to compare against
  let head = repo.head().context("Failed to get HEAD reference")?;
  let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;
  let head_tree = head_commit.tree().context("Failed to get HEAD tree")?;

  // Compare the index with HEAD tree to see if there are staged changes
  let diff = repo
    .diff_tree_to_index(Some(&head_tree), Some(&index), None)
    .context("Failed to create diff between HEAD and index")?;

  // If there are any deltas (changes), then there are staged changes
  Ok(diff.deltas().len() > 0)
}

#[cfg(test)]
mod tests {
  use chrono::Utc;

  use super::*;

  fn create_test_candidate() -> CommitCandidate {
    CommitCandidate {
      hash: "abc123def456".to_string(),
      short_hash: "abc123".to_string(),
      message: "Test commit".to_string(),
      author: "test_user".to_string(),
      date: Utc::now(),
      is_current_user: true,
      jira_issue: None,
      score: 0.8,
    }
  }

  #[test]
  fn test_commit_candidate_creation() {
    let candidate = create_test_candidate();
    assert_eq!(candidate.hash, "abc123def456");
    assert_eq!(candidate.short_hash, "abc123");
    assert_eq!(candidate.message, "Test commit");
  }
}
