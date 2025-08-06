//! # Commit Collection and Filtering
//!
//! This module handles the discovery and initial filtering of commit candidates
//! that can serve as targets for fixup commits. It interfaces with Git to
//! retrieve commit history and extracts relevant metadata for scoring and
//! selection.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use git2::Repository;

use crate::cli::fixup::FixupArgs;

/// Represents a commit candidate for fixup
#[derive(Debug, Clone)]
pub struct CommitCandidate {
  pub hash: String,
  pub short_hash: String,
  pub message: String,
  pub author: String,
  pub date: DateTime<Utc>,
  pub is_current_user: bool,
  pub jira_issue: Option<String>,
  pub score: f64,
}

/// Collects commit candidates from the current Git branch.
///
/// This function executes `git log` to retrieve recent commits and parses them
/// into [`CommitCandidate`] structures for further processing. The collection
/// can be filtered by author and limited by time window and count.
///
/// # Arguments
///
/// * `repo_path` - Path to the Git repository root
/// * `args` - Configuration parameters controlling the collection process
///
/// # Returns
///
/// A vector of [`CommitCandidate`] objects representing potential fixup
/// targets, ordered by Git's default chronological ordering (newest first).
///
/// # Errors
///
/// Returns an error if:
/// - Git is not available or the command fails
/// - The repository path is invalid
/// - Git user configuration is missing
/// - Commit parsing fails due to unexpected format
pub fn collect_commits(repo_path: &Path, args: &FixupArgs) -> Result<Vec<CommitCandidate>> {
  let repo = Repository::open(repo_path).context("Failed to open git repository")?;

  let current_user = get_current_git_user(&repo)?;
  let current_jira_issue = twig_core::get_current_branch_jira_issue().unwrap_or(None);

  tracing::debug!("Current git user: {}", current_user);
  tracing::debug!("Current branch Jira issue: {:?}", current_jira_issue);

  // Calculate the cutoff timestamp for filtering by days
  let since_timestamp = (Utc::now() - Duration::days(args.days as i64)).timestamp();

  let mut revwalk = repo.revwalk().context("Failed to create revwalk")?;

  // Use default sorting (topological) which gives newest commits first for linear
  // history Explicit sorting with git2::Sort::TIME gives oldest first, which is
  // opposite of what we want
  revwalk.push_head().context("Failed to push HEAD to revwalk")?;

  let mut candidates = Vec::new();
  let mut count = 0;

  for oid in revwalk {
    if count >= args.limit {
      break;
    }

    let oid = oid.context("Failed to get commit OID")?;
    let commit = repo.find_commit(oid).context("Failed to find commit")?;

    let commit_time = commit.time();

    // Filter by date - skip commits older than the specified days
    if commit_time.seconds() < since_timestamp {
      continue;
    }

    let author = commit.author();
    let author_name = author.name().unwrap_or("").to_string();

    // Filter by author if needed
    if !args.all_authors && author_name != current_user {
      continue;
    }

    let message = commit.message().unwrap_or("").to_string();
    let hash = commit.id().to_string();
    let short_hash = format!("{:.7}", commit.id());

    // Filter out fixup commits unless explicitly requested
    if !args.include_fixups && is_fixup_commit(&message) {
      continue;
    }

    // Convert git2::Time to DateTime<Utc>
    let date = DateTime::from_timestamp(commit_time.seconds(), 0)
      .unwrap_or_else(|| {
        tracing::warn!("Failed to parse commit timestamp: {}", commit_time.seconds());
        Utc::now()
      })
      .with_timezone(&Utc);

    // Extract Jira issue from commit message
    let jira_issue = extract_jira_issue_from_message(&message);

    let candidate = CommitCandidate {
      hash,
      short_hash,
      message,
      is_current_user: author_name == current_user,
      author: author_name,
      date,
      jira_issue,
      score: 0.0, // Will be calculated by scorer
    };

    candidates.push(candidate);
    count += 1;
  }

  tracing::debug!("Collected {} commit candidates", candidates.len());
  Ok(candidates)
}

/// Get the current git user name
fn get_current_git_user(repo: &Repository) -> Result<String> {
  let config = repo.config().context("Failed to get git config")?;

  let user_name = config
    .get_string("user.name")
    .context("Failed to get user.name from git config")?;

  Ok(user_name)
}

/// Extract Jira issue key from commit message using flexible parser
fn extract_jira_issue_from_message(message: &str) -> Option<String> {
  use twig_core::get_config_dirs;
  use twig_core::jira_parser::JiraTicketParser;

  // Load Jira configuration and create parser
  let config_dirs = get_config_dirs().ok()?;
  let jira_config = config_dirs.load_jira_config().ok()?;
  let parser = JiraTicketParser::new(jira_config);

  // Use the parser's commit message extraction method
  parser.extract_from_commit_message(message)
}

/// Check if a commit message indicates a fixup commit
fn is_fixup_commit(message: &str) -> bool {
  message.starts_with("fixup!")
}

#[cfg(test)]
mod tests {
  use twig_test_utils::git::{GitRepoTestGuard, create_commit_with_author};

  use super::*;

  #[test]
  fn test_extract_jira_issue_from_message() {
    assert_eq!(
      extract_jira_issue_from_message("PROJ-123: Fix the bug"),
      Some("PROJ-123".to_string())
    );

    assert_eq!(
      extract_jira_issue_from_message("TEAM-456: Add new feature"),
      Some("TEAM-456".to_string())
    );

    assert_eq!(extract_jira_issue_from_message("Fix the bug without issue"), None);

    assert_eq!(
      extract_jira_issue_from_message("Some text PROJ-123: not at start"),
      None
    );
  }

  #[test]
  fn test_get_current_git_user_with_default_config() {
    let git_repo = GitRepoTestGuard::new();

    // The GitRepoTestGuard sets up "Twig Test User" as the default user
    let result = get_current_git_user(&git_repo.repo);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Twig Test User");
  }

  #[test]
  fn test_get_current_git_user_with_custom_config() {
    let git_repo = GitRepoTestGuard::new();

    // Set a custom user name
    let mut config = git_repo.repo.config().expect("Failed to get config");
    config
      .set_str("user.name", "Custom Test User")
      .expect("Failed to set custom user.name");

    let result = get_current_git_user(&git_repo.repo);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Custom Test User");
  }

  #[test]
  fn test_get_current_git_user_with_unicode_name() {
    let git_repo = GitRepoTestGuard::new();

    // Set a user name with unicode characters
    let mut config = git_repo.repo.config().expect("Failed to get config");
    config
      .set_str("user.name", "José García-Müller")
      .expect("Failed to set unicode user.name");

    let result = get_current_git_user(&git_repo.repo);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "José García-Müller");
  }

  #[test]
  fn test_get_current_git_user_with_spaces_and_special_chars() {
    let git_repo = GitRepoTestGuard::new();

    // Set a user name with spaces and special characters
    let mut config = git_repo.repo.config().expect("Failed to get config");
    config
      .set_str("user.name", "John O'Connor-Smith Jr.")
      .expect("Failed to set special char user.name");

    let result = get_current_git_user(&git_repo.repo);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "John O'Connor-Smith Jr.");
  }
  #[test]
  fn test_collect_commits_basic() {
    let git_repo = GitRepoTestGuard::new();

    // Create some test commits with different authors
    create_commit_with_author(
      &git_repo.repo,
      "file1.txt",
      "content1",
      "Initial commit",
      "Test User",
      "test@example.com",
    )
    .expect("Failed to create initial commit");
    create_commit_with_author(
      &git_repo.repo,
      "file2.txt",
      "content2",
      "PROJ-123: Add feature",
      "Test User",
      "test@example.com",
    )
    .expect("Failed to create feature commit");
    create_commit_with_author(
      &git_repo.repo,
      "file3.txt",
      "content3",
      "Fix bug",
      "Other User",
      "other@example.com",
    )
    .expect("Failed to create bug fix commit");

    // Create FixupArgs with default values
    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: true,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    // Test collect_commits
    let result = collect_commits(git_repo.repo.path().parent().unwrap(), &args);

    assert!(result.is_ok());
    let candidates = result.unwrap();

    // Should have collected all 3 commits
    assert_eq!(candidates.len(), 3);

    // Verify the commits are in reverse chronological order (newest first)
    assert_eq!(candidates[0].message, "Fix bug");
    assert_eq!(candidates[1].message, "PROJ-123: Add feature");
    assert_eq!(candidates[2].message, "Initial commit");

    // Verify Jira issue extraction
    assert_eq!(candidates[1].jira_issue, Some("PROJ-123".to_string()));
    assert_eq!(candidates[0].jira_issue, None);
    assert_eq!(candidates[2].jira_issue, None);

    // Verify author information
    assert_eq!(candidates[0].author, "Other User");
    assert_eq!(candidates[1].author, "Test User");
    assert_eq!(candidates[2].author, "Test User");
  }

  #[test]
  fn test_collect_commits_with_author_filter() {
    let git_repo = GitRepoTestGuard::new();

    // Create commits from different authors
    create_commit_with_author(
      &git_repo.repo,
      "file1.txt",
      "content1",
      "Commit by test user",
      "Twig Test User",
      "test@example.com",
    )
    .expect("Failed to create test user commit");
    create_commit_with_author(
      &git_repo.repo,
      "file2.txt",
      "content2",
      "Commit by other user",
      "Other User",
      "other@example.com",
    )
    .expect("Failed to create other user commit");

    // Create FixupArgs with author filtering enabled (all_authors = false)
    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: false,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    // Test collect_commits
    let result = collect_commits(git_repo.repo.path().parent().unwrap(), &args);

    assert!(result.is_ok());
    let candidates = result.unwrap();

    // Should only have 1 commit from the current user (Twig Test User)
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].message, "Commit by test user");
    assert_eq!(candidates[0].author, "Twig Test User");
    assert!(candidates[0].is_current_user);
  }

  #[test]
  fn test_collect_commits_with_limit() {
    let git_repo = GitRepoTestGuard::new();

    // Create more commits than the limit
    for i in 1..=5 {
      create_commit_with_author(
        &git_repo.repo,
        &format!("file{i}.txt"),
        &format!("content{i}",),
        &format!("Commit {i}"),
        "Twig Test User",
        "test@example.com",
      )
      .expect(&format!("Failed to create commit {i}"));
    }

    // Create FixupArgs with a limit of 3
    let args = FixupArgs {
      limit: 3,
      days: 30,
      all_authors: true,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    // Test collect_commits
    let result = collect_commits(git_repo.repo.path().parent().unwrap(), &args);

    assert!(result.is_ok());
    let candidates = result.unwrap();

    // Should only have 3 commits due to the limit
    assert_eq!(candidates.len(), 3);

    // Should be the 3 most recent commits
    assert_eq!(candidates[0].message, "Commit 5");
    assert_eq!(candidates[1].message, "Commit 4");
    assert_eq!(candidates[2].message, "Commit 3");
  }

  #[test]
  fn test_is_fixup_commit() {
    assert!(is_fixup_commit("fixup! Fix the bug"));
    assert!(is_fixup_commit("fixup! PROJ-123: Add feature"));
    assert!(!is_fixup_commit("Fix the bug"));
    assert!(!is_fixup_commit("PROJ-123: Add feature"));
    assert!(!is_fixup_commit(""));
  }

  #[test]
  fn test_collect_commits_excludes_fixups_by_default() {
    let git_repo = GitRepoTestGuard::new();

    // Create regular commits and fixup commits
    create_commit_with_author(
      &git_repo.repo,
      "file1.txt",
      "content1",
      "Regular commit",
      "Twig Test User",
      "test@example.com",
    )
    .expect("Failed to create regular commit");
    create_commit_with_author(
      &git_repo.repo,
      "file2.txt",
      "content2",
      "fixup! Regular commit",
      "Twig Test User",
      "test@example.com",
    )
    .expect("Failed to create fixup commit");
    create_commit_with_author(
      &git_repo.repo,
      "file3.txt",
      "content3",
      "Another regular commit",
      "Twig Test User",
      "test@example.com",
    )
    .expect("Failed to create another regular commit");

    // Test with include_fixups = false (default)
    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: true,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    let result = collect_commits(git_repo.repo.path().parent().unwrap(), &args);
    assert!(result.is_ok());
    let candidates = result.unwrap();

    // Should only have 2 commits (excluding the fixup commit)
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].message, "Another regular commit");
    assert_eq!(candidates[1].message, "Regular commit");

    // Test with include_fixups = true
    let args_with_fixups = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: true,
      include_fixups: true,
      dry_run: false,
      vim_mode: false,
    };

    let result_with_fixups = collect_commits(git_repo.repo.path().parent().unwrap(), &args_with_fixups);
    assert!(result_with_fixups.is_ok());
    let candidates_with_fixups = result_with_fixups.unwrap();

    // Should have all 3 commits (including the fixup commit)
    assert_eq!(candidates_with_fixups.len(), 3);
    assert_eq!(candidates_with_fixups[0].message, "Another regular commit");
    assert_eq!(candidates_with_fixups[1].message, "fixup! Regular commit");
    assert_eq!(candidates_with_fixups[2].message, "Regular commit");
  }
}
