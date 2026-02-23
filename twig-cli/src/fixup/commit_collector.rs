//! # Commit Collection and Filtering
//!
//! This module handles the discovery and initial filtering of commit candidates
//! that can serve as targets for fixup commits. It interfaces with Git to
//! retrieve commit history and extracts relevant metadata for scoring and
//! selection.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use git2::{Oid, Repository};
use twig_core::state::RepoState;

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
  pub is_branch_unique: bool,
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

  // Create Jira parser once for the entire collection process
  let jira_parser = twig_core::create_jira_parser();

  tracing::debug!("Current git user: {}", current_user);
  tracing::debug!("Current branch Jira issue: {:?}", current_jira_issue);

  // Resolve comparison branch for branch uniqueness detection
  let current_branch = get_current_branch_name(&repo);
  let comparison_branch = current_branch
    .as_ref()
    .and_then(|branch| resolve_comparison_branch(&repo, repo_path, branch));
  let comparison_tip = comparison_branch
    .as_ref()
    .and_then(|branch| get_comparison_branch_tip(&repo, branch));

  tracing::debug!(
    "Branch uniqueness detection: current={:?}, comparison={:?}, comparison_tip={:?}",
    current_branch,
    comparison_branch,
    comparison_tip
  );

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

    let message = commit.message().unwrap_or("").to_string();

    // Filter out fixup commits unless explicitly requested
    if !args.include_fixups && is_fixup_commit(&message) {
      continue;
    }

    // Determine if this commit is unique to the current branch BEFORE author
    // filtering. Branch-unique commits should always be included regardless
    // of author, since they are the most relevant fixup targets.
    // A commit is branch-unique if it's NOT reachable from the comparison branch tip
    // (i.e., the comparison tip is NOT a descendant of this commit, and they're not equal)
    let is_branch_unique = comparison_tip.is_some_and(|tip| {
      // The commit is reachable from the comparison tip if:
      // - The tip is a descendant of the commit (commit is an ancestor of tip), OR
      // - The commit equals the tip
      // So branch-unique means: tip is NOT a descendant AND not equal
      oid != tip && !repo.graph_descendant_of(tip, oid).unwrap_or(true)
    });

    let author = commit.author();
    let author_name = author.name().unwrap_or("").to_string();

    // Filter by author if needed, but always keep branch-unique commits
    if !args.all_authors && !is_branch_unique && author_name != current_user {
      continue;
    }

    let hash = commit.id().to_string();
    let short_hash = format!("{:.7}", commit.id());

    // Convert git2::Time to DateTime<Utc>
    let date = DateTime::from_timestamp(commit_time.seconds(), 0)
      .unwrap_or_else(|| {
        tracing::warn!("Failed to parse commit timestamp: {}", commit_time.seconds());
        Utc::now()
      })
      .with_timezone(&Utc);

    // Extract Jira issue from commit message
    let jira_issue = jira_parser
      .as_ref()
      .and_then(|parser| extract_jira_issue_from_message(parser, &message));

    let candidate = CommitCandidate {
      hash,
      short_hash,
      message,
      is_current_user: author_name == current_user,
      author: author_name,
      date,
      jira_issue,
      is_branch_unique,
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

/// Extract Jira issue key from commit message using the provided parser
fn extract_jira_issue_from_message(parser: &twig_core::jira_parser::JiraTicketParser, message: &str) -> Option<String> {
  // Use the parser's commit message extraction method
  parser.extract_from_commit_message(message)
}

/// Check if a commit message indicates a fixup commit
fn is_fixup_commit(message: &str) -> bool {
  message.starts_with("fixup!")
}

/// Resolves the comparison branch for determining branch uniqueness.
///
/// The comparison branch is determined with the following priority:
/// 1. Configured dependency parent (via RepoState::get_dependency_parents())
/// 2. Default root branch (via RepoState::get_default_root())
/// 3. Fallback to origin/main or origin/master
///
/// # Arguments
///
/// * `repo` - The git2 Repository
/// * `repo_path` - Path to the repository root
/// * `current_branch` - Name of the current branch
///
/// # Returns
///
/// The resolved comparison branch reference, or None if no suitable branch found.
fn resolve_comparison_branch(repo: &Repository, repo_path: &Path, current_branch: &str) -> Option<String> {
  // Try to load repo state for dependency information
  if let Ok(state) = RepoState::load(repo_path) {
    // Priority 1: Check for configured dependency parent
    let parents = state.get_dependency_parents(current_branch);
    if let Some(parent) = parents.first() {
      tracing::debug!("Using dependency parent '{}' as comparison branch", parent);
      // Try local branch first, then remote tracking
      if repo.find_branch(parent, git2::BranchType::Local).is_ok() {
        return Some(parent.to_string());
      }
      // Try as remote tracking branch
      let remote_ref = format!("origin/{parent}");
      if repo.find_reference(&format!("refs/remotes/{remote_ref}")).is_ok() {
        return Some(remote_ref);
      }
    }

    // Priority 2: Use default root branch
    if let Some(default_root) = state.get_default_root() {
      tracing::debug!("Using default root '{}' as comparison branch", default_root);
      if repo.find_branch(default_root, git2::BranchType::Local).is_ok() {
        return Some(default_root.to_string());
      }
      let remote_ref = format!("origin/{default_root}");
      if repo.find_reference(&format!("refs/remotes/{remote_ref}")).is_ok() {
        return Some(remote_ref);
      }
    }
  }

  // Priority 3: Fallback to origin/main or origin/master
  for fallback in &["origin/main", "origin/master"] {
    if repo.find_reference(&format!("refs/remotes/{fallback}")).is_ok() {
      tracing::debug!("Using fallback '{}' as comparison branch", fallback);
      return Some((*fallback).to_string());
    }
  }

  tracing::debug!("No comparison branch found");
  None
}

/// Gets the tip commit OID of the comparison branch.
///
/// # Arguments
///
/// * `repo` - The git2 Repository
/// * `comparison_branch` - Name of the branch to compare against
///
/// # Returns
///
/// The OID of the comparison branch tip, or None if it cannot be resolved.
fn get_comparison_branch_tip(repo: &Repository, comparison_branch: &str) -> Option<Oid> {
  if comparison_branch.starts_with("origin/") {
    // Remote tracking branch
    let refname = format!("refs/remotes/{comparison_branch}");
    repo.find_reference(&refname).ok()?.target()
  } else {
    // Local branch
    repo
      .find_branch(comparison_branch, git2::BranchType::Local)
      .ok()?
      .get()
      .target()
  }
}

/// Gets the current branch name from the repository.
fn get_current_branch_name(repo: &Repository) -> Option<String> {
  let head = repo.head().ok()?;
  if head.is_branch() {
    head.shorthand().map(|s| s.to_string())
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use twig_test_utils::git::{GitRepoTestGuard, create_commit_with_author};

  use super::*;

  #[test]
  fn test_extract_jira_issue_from_message() {
    use twig_core::jira_parser::{JiraParsingConfig, JiraTicketParser};

    let parser = JiraTicketParser::new(JiraParsingConfig::default());

    assert_eq!(
      extract_jira_issue_from_message(&parser, "PROJ-123: Fix the bug"),
      Some("PROJ-123".to_string())
    );

    assert_eq!(
      extract_jira_issue_from_message(&parser, "TEAM-456: Add new feature"),
      Some("TEAM-456".to_string())
    );

    assert_eq!(
      extract_jira_issue_from_message(&parser, "Fix the bug without issue"),
      None
    );

    assert_eq!(
      extract_jira_issue_from_message(&parser, "Some text PROJ-123: not at start"),
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
  fn test_branch_unique_commits_bypass_author_filter() {
    use twig_test_utils::git::{checkout_branch, create_branch, ensure_main_branch};

    let git_repo = GitRepoTestGuard::new();

    // Set up a main branch with a shared commit
    create_commit_with_author(
      &git_repo.repo,
      "shared.txt",
      "shared content",
      "Shared commit on main",
      "Twig Test User",
      "test@example.com",
    )
    .expect("Failed to create shared commit");

    ensure_main_branch(&git_repo.repo).expect("Failed to ensure main branch");

    // Create a fake origin/main remote tracking ref so resolve_comparison_branch
    // can find a comparison target (test repos don't have real remotes)
    let main_tip = git_repo.repo.head().unwrap().target().unwrap();
    git_repo
      .repo
      .reference("refs/remotes/origin/main", main_tip, true, "fake remote tracking ref")
      .expect("Failed to create fake origin/main ref");

    // Create a feature branch from main
    create_branch(&git_repo.repo, "feature", None).expect("Failed to create feature branch");
    checkout_branch(&git_repo.repo, "feature").expect("Failed to checkout feature branch");

    // Add a commit by a DIFFERENT author (e.g., an AI assistant)
    create_commit_with_author(
      &git_repo.repo,
      "feature.txt",
      "feature content",
      "Feature commit by other author",
      "Claude",
      "claude@example.com",
    )
    .expect("Failed to create feature commit");

    // Collect commits with author filter ON (all_authors = false)
    let args = FixupArgs {
      limit: 20,
      days: 30,
      all_authors: false,
      include_fixups: false,
      dry_run: false,
      vim_mode: false,
    };

    let result = collect_commits(git_repo.repo.path().parent().unwrap(), &args);
    assert!(result.is_ok());
    let candidates = result.unwrap();

    // The branch-unique commit by "Claude" should still be included
    // even though it doesn't match the current user, because it's
    // the only commit unique to this branch
    let branch_unique_commits: Vec<_> = candidates.iter().filter(|c| c.is_branch_unique).collect();
    assert_eq!(
      branch_unique_commits.len(),
      1,
      "Should have exactly one branch-unique commit"
    );
    assert_eq!(branch_unique_commits[0].message, "Feature commit by other author");
    assert_eq!(branch_unique_commits[0].author, "Claude");
    assert!(!branch_unique_commits[0].is_current_user);
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
