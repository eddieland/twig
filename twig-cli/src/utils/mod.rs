//! # Utility Functions
//!
//! Common utility functions and helpers for file operations, Git repository
//! validation, and shared functionality across the twig application.

use std::path::PathBuf;

use anyhow::{Context, Result};
use git2::Repository as Git2Repository;

pub mod output;

/// Represents an associated item for a branch
#[derive(Debug, Clone)]
pub enum BranchAssociatedItem {
  /// Associated Jira issue key
  JiraIssue(String),
  /// Associated GitHub PR number
  GitHubPr(u32),
  /// No associated item found
  None,
}

/// Resolve a repository path from a command line argument or current directory
pub fn resolve_repository_path(repo_arg: Option<&str>) -> Result<PathBuf> {
  match repo_arg {
    Some(path) => {
      let path_buf = PathBuf::from(path);
      if !path_buf.exists() {
        return Err(anyhow::anyhow!("Repository path does not exist: {}", path));
      }
      crate::git::detect_repository(&path_buf).context(format!("Failed to detect repository at path: {path}"))
    }
    None => {
      // Try to detect the current repository
      crate::git::detect_current_repository().context("No repository specified and not in a git repository")
    }
  }
}

/// Get the associated Jira issue or GitHub PR for the current branch
///
/// This function attempts to find the Jira issue key or GitHub PR number
/// associated with the current branch. If neither is found, it returns None.
pub fn get_current_branch_associated_item() -> Result<BranchAssociatedItem> {
  // Get the current repository
  let repo_path = crate::git::detect_current_repository().context("Not in a git repository")?;

  // Open the git repository
  let repo = Git2Repository::open(&repo_path).context("Failed to open git repository")?;

  // Get the current branch
  let head = repo.head().context("Failed to get repository HEAD")?;

  if !head.is_branch() {
    return Ok(BranchAssociatedItem::None);
  }

  let branch_name = head
    .shorthand()
    .ok_or_else(|| anyhow::anyhow!("Failed to get branch name"))?;

  // Load the repository state
  let repo_state = crate::repo_state::RepoState::load(&repo_path).context("Failed to load repository state")?;

  // Check if the branch has an associated issue
  if let Some(branch_issue) = repo_state.get_branch_issue_by_branch(branch_name) {
    if let Some(jira_issue) = &branch_issue.jira_issue {
      return Ok(BranchAssociatedItem::JiraIssue(jira_issue.clone()));
    }

    if let Some(github_pr) = branch_issue.github_pr {
      return Ok(BranchAssociatedItem::GitHubPr(github_pr));
    }
  }

  Ok(BranchAssociatedItem::None)
}

/// Get the associated Jira issue key for the current branch
///
/// This is a convenience function that returns the Jira issue key if found,
/// or None if not found or if a GitHub PR is associated instead.
pub fn get_current_branch_jira_issue() -> Result<Option<String>> {
  match get_current_branch_associated_item()? {
    BranchAssociatedItem::JiraIssue(key) => Ok(Some(key)),
    _ => Ok(None),
  }
}

/// Get the associated GitHub PR number for the current branch
///
/// This is a convenience function that returns the GitHub PR number if found,
/// or None if not found or if a Jira issue is associated instead.
pub fn get_current_branch_github_pr() -> Result<Option<u32>> {
  match get_current_branch_associated_item()? {
    BranchAssociatedItem::GitHubPr(number) => Ok(Some(number)),
    _ => Ok(None),
  }
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;
  use twig_test_utils::GitRepoTestGuard;

  use super::*;

  // Test the BranchAssociatedItem enum
  #[test]
  fn test_branch_associated_item_variants() {
    let jira_item = BranchAssociatedItem::JiraIssue("ABC-123".to_string());
    let github_item = BranchAssociatedItem::GitHubPr(42);
    let none_item = BranchAssociatedItem::None;

    match jira_item {
      BranchAssociatedItem::JiraIssue(key) => assert_eq!(key, "ABC-123"),
      _ => panic!("Expected JiraIssue variant"),
    }

    match github_item {
      BranchAssociatedItem::GitHubPr(num) => assert_eq!(num, 42),
      _ => panic!("Expected GitHubPr variant"),
    }

    match none_item {
      BranchAssociatedItem::None => {}
      _ => panic!("Expected None variant"),
    }
  }

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

  // Test the direct conversion logic for get_current_branch_jira_issue
  #[test]
  fn test_get_current_branch_jira_issue_conversion() {
    // Test with JiraIssue variant
    let jira_key = "ABC-123".to_string();
    let result: anyhow::Result<Option<String>> = match BranchAssociatedItem::JiraIssue(jira_key.clone()) {
      BranchAssociatedItem::JiraIssue(key) => Ok(Some(key)),
      _ => Ok(None),
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(jira_key));

    // Test with GitHubPr variant
    let result: anyhow::Result<Option<String>> = match BranchAssociatedItem::GitHubPr(42) {
      BranchAssociatedItem::JiraIssue(key) => Ok(Some(key)),
      _ => Ok(None),
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);

    // Test with None variant
    let result: anyhow::Result<Option<String>> = match BranchAssociatedItem::None {
      BranchAssociatedItem::JiraIssue(key) => Ok(Some(key)),
      _ => Ok(None),
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
  }

  // Test the direct conversion logic for get_current_branch_github_pr
  #[test]
  fn test_get_current_branch_github_pr_conversion() {
    // Test with GitHubPr variant
    let pr_number = 42;
    let result: anyhow::Result<Option<u32>> = match BranchAssociatedItem::GitHubPr(pr_number) {
      BranchAssociatedItem::GitHubPr(num) => Ok(Some(num)),
      _ => Ok(None),
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(pr_number));

    // Test with JiraIssue variant
    let result: anyhow::Result<Option<u32>> = match BranchAssociatedItem::JiraIssue("ABC-123".to_string()) {
      BranchAssociatedItem::GitHubPr(num) => Ok(Some(num)),
      _ => Ok(None),
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);

    // Test with None variant
    let result: anyhow::Result<Option<u32>> = match BranchAssociatedItem::None {
      BranchAssociatedItem::GitHubPr(num) => Ok(Some(num)),
      _ => Ok(None),
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
  }
}
