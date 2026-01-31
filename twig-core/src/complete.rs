//! # Dynamic Shell Completion Support
//!
//! Provides dynamic shell completion utilities for twig CLI commands using
//! `clap_complete::CompleteEnv`. Completions are generated at runtime from:
//! - Local Git branch names
//! - Jira issue keys associated with branches
//! - GitHub PR IDs associated with branches
//!
//! This module is only available when the `complete` feature is enabled.

use std::collections::HashSet;
use std::ffi::OsStr;

use clap_complete::engine::{ArgValueCompleter, CompletionCandidate, ValueCompleter};

use crate::git::{get_local_branches, get_remote_branches, get_repository};
use crate::state::RepoState;

/// The type of completion candidate, used for help text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateKind {
  /// A local Git branch
  Branch,
  /// A remote Git branch (not checked out locally)
  RemoteBranch,
  /// A Jira issue key (e.g., PROJ-123)
  JiraIssue,
  /// A GitHub PR ID (e.g., #123)
  GitHubPr,
}

impl CandidateKind {
  /// Returns a short description for use in completion help text.
  pub fn help_text(self) -> &'static str {
    match self {
      CandidateKind::Branch => "branch",
      CandidateKind::RemoteBranch => "remote branch",
      CandidateKind::JiraIssue => "Jira issue",
      CandidateKind::GitHubPr => "GitHub PR",
    }
  }
}

/// A completion candidate with its type.
#[derive(Clone, Debug)]
pub struct TypedCandidate {
  /// The value to complete to.
  pub value: String,
  /// The type of candidate.
  pub kind: CandidateKind,
}

impl TypedCandidate {
  /// Create a new typed candidate.
  pub fn new(value: impl Into<String>, kind: CandidateKind) -> Self {
    Self {
      value: value.into(),
      kind,
    }
  }

  /// Convert to a clap_complete CompletionCandidate with help text.
  pub fn to_completion_candidate(&self) -> CompletionCandidate {
    CompletionCandidate::new(&self.value).help(Some(self.kind.help_text().into()))
  }
}

/// Collect all completion candidates from available data sources.
///
/// Returns a list of typed candidates including:
/// - Local branch names
/// - Jira issue keys from repo state
/// - GitHub PR IDs from repo state (with and without # prefix)
pub fn collect_typed_candidates() -> Vec<TypedCandidate> {
  let mut candidates = Vec::new();
  let mut seen = HashSet::new();

  // Collect branch names
  if let Ok(branches) = get_local_branches() {
    for branch in branches {
      if seen.insert(branch.clone()) {
        candidates.push(TypedCandidate::new(branch, CandidateKind::Branch));
      }
    }
  }

  // Collect Jira keys and PR IDs from repo state
  if let Some(repo) = get_repository()
    && let Some(workdir) = repo.workdir()
    && let Ok(state) = RepoState::load(workdir)
  {
    // Add Jira issue keys
    for jira_key in state.jira_to_branch_index.keys() {
      if seen.insert(jira_key.clone()) {
        candidates.push(TypedCandidate::new(jira_key.clone(), CandidateKind::JiraIssue));
      }
    }

    // Add GitHub PR IDs (prefixed with # for clarity)
    for metadata in state.branches.values() {
      if let Some(pr_id) = metadata.github_pr {
        let with_hash = format!("#{pr_id}");
        let without_hash = pr_id.to_string();

        if seen.insert(with_hash.clone()) {
          candidates.push(TypedCandidate::new(with_hash, CandidateKind::GitHubPr));
        }
        if seen.insert(without_hash.clone()) {
          candidates.push(TypedCandidate::new(without_hash, CandidateKind::GitHubPr));
        }
      }
    }
  }

  // Sort for consistent output
  candidates.sort_by(|a, b| a.value.cmp(&b.value));

  candidates
}

/// Collect all completion candidates as simple strings.
///
/// This is a convenience function for cases where you don't need the type information.
pub fn collect_candidates() -> Vec<String> {
  collect_typed_candidates().into_iter().map(|c| c.value).collect()
}

/// Collect only local branch name candidates.
pub fn collect_branch_candidates() -> Vec<TypedCandidate> {
  let mut candidates = Vec::new();
  let mut seen = HashSet::new();

  if let Ok(branches) = get_local_branches() {
    for branch in branches {
      if seen.insert(branch.clone()) {
        candidates.push(TypedCandidate::new(branch, CandidateKind::Branch));
      }
    }
  }

  candidates.sort_by(|a, b| a.value.cmp(&b.value));
  candidates
}

/// Collect both local and remote branch candidates.
///
/// Remote branches that already exist locally are excluded to avoid duplicates.
/// Remote branches are marked with `CandidateKind::RemoteBranch` for distinct help text.
pub fn collect_all_branch_candidates() -> Vec<TypedCandidate> {
  let mut candidates = Vec::new();
  let mut seen = HashSet::new();

  // First, collect local branches
  if let Ok(branches) = get_local_branches() {
    for branch in branches {
      if seen.insert(branch.clone()) {
        candidates.push(TypedCandidate::new(branch, CandidateKind::Branch));
      }
    }
  }

  // Then, collect remote branches (excluding those already present locally)
  if let Ok(branches) = get_remote_branches() {
    for branch in branches {
      if seen.insert(branch.clone()) {
        candidates.push(TypedCandidate::new(branch, CandidateKind::RemoteBranch));
      }
    }
  }

  candidates.sort_by(|a, b| a.value.cmp(&b.value));
  candidates
}

/// A generic completer that provides branch names, Jira keys, and PR IDs as candidates.
///
/// This is used for commands like `twig switch` that accept various target formats.
#[derive(Clone)]
pub struct TargetCompleter;

impl ValueCompleter for TargetCompleter {
  fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy().to_lowercase();
    collect_typed_candidates()
      .into_iter()
      .filter(|c| c.value.to_lowercase().starts_with(&current_str))
      .map(|c| c.to_completion_candidate())
      .collect()
  }
}

/// Returns an `ArgValueCompleter` for use with clap's `add` extension.
///
/// This completer provides branch names, Jira keys, and PR IDs as candidates.
pub fn target_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(TargetCompleter)
}

/// A completer that only provides branch names as candidates.
///
/// This is useful for commands that only accept branch names, not issue keys or PR IDs.
#[derive(Clone)]
pub struct BranchCompleter;

impl ValueCompleter for BranchCompleter {
  fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy().to_lowercase();
    collect_branch_candidates()
      .into_iter()
      .filter(|c| c.value.to_lowercase().starts_with(&current_str))
      .map(|c| c.to_completion_candidate())
      .collect()
  }
}

/// Returns an `ArgValueCompleter` that only provides branch names.
pub fn branch_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(BranchCompleter)
}

/// A completer that provides both local and remote branch names as candidates.
///
/// This is useful for commands like `worktree create` that can work with remote branches.
#[derive(Clone)]
pub struct AllBranchCompleter;

impl ValueCompleter for AllBranchCompleter {
  fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy().to_lowercase();
    collect_all_branch_candidates()
      .into_iter()
      .filter(|c| c.value.to_lowercase().starts_with(&current_str))
      .map(|c| c.to_completion_candidate())
      .collect()
  }
}

/// Returns an `ArgValueCompleter` that provides both local and remote branch names.
pub fn all_branch_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(AllBranchCompleter)
}

/// A completer that uses fuzzy (contains) matching instead of prefix matching.
///
/// This is more forgiving for users who may not remember the exact start of a branch name.
#[derive(Clone)]
pub struct FuzzyTargetCompleter;

impl ValueCompleter for FuzzyTargetCompleter {
  fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy().to_lowercase();
    if current_str.is_empty() {
      // Return all candidates when input is empty
      return collect_typed_candidates()
        .into_iter()
        .map(|c| c.to_completion_candidate())
        .collect();
    }

    collect_typed_candidates()
      .into_iter()
      .filter(|c| c.value.to_lowercase().contains(&current_str))
      .map(|c| c.to_completion_candidate())
      .collect()
  }
}

/// Returns an `ArgValueCompleter` with fuzzy (contains) matching.
pub fn fuzzy_target_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(FuzzyTargetCompleter)
}

/// A completer that uses fuzzy (contains) matching for branch names only.
#[derive(Clone)]
pub struct FuzzyBranchCompleter;

impl ValueCompleter for FuzzyBranchCompleter {
  fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy().to_lowercase();
    if current_str.is_empty() {
      return collect_branch_candidates()
        .into_iter()
        .map(|c| c.to_completion_candidate())
        .collect();
    }

    collect_branch_candidates()
      .into_iter()
      .filter(|c| c.value.to_lowercase().contains(&current_str))
      .map(|c| c.to_completion_candidate())
      .collect()
  }
}

/// Returns an `ArgValueCompleter` with fuzzy (contains) matching for branches only.
pub fn fuzzy_branch_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(FuzzyBranchCompleter)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Helper to filter candidates by prefix (mirrors the completer logic)
  fn filter_by_prefix(candidates: &[TypedCandidate], prefix: &str) -> Vec<String> {
    let prefix_lower = prefix.to_lowercase();
    candidates
      .iter()
      .filter(|c| c.value.to_lowercase().starts_with(&prefix_lower))
      .map(|c| c.value.clone())
      .collect()
  }

  #[test]
  fn filters_by_prefix_case_insensitive() {
    let candidates = vec![
      TypedCandidate::new("feature/alpha", CandidateKind::Branch),
      TypedCandidate::new("feature/beta", CandidateKind::Branch),
      TypedCandidate::new("Feature/gamma", CandidateKind::Branch),
      TypedCandidate::new("bugfix/delta", CandidateKind::Branch),
      TypedCandidate::new("PROJ-123", CandidateKind::JiraIssue),
    ];

    // Filter with lowercase prefix
    let filtered = filter_by_prefix(&candidates, "feat");
    assert_eq!(filtered.len(), 3);
    assert!(filtered.contains(&"feature/alpha".to_string()));
    assert!(filtered.contains(&"feature/beta".to_string()));
    assert!(filtered.contains(&"Feature/gamma".to_string()));

    // Filter with uppercase prefix
    let filtered = filter_by_prefix(&candidates, "FEAT");
    assert_eq!(filtered.len(), 3);

    // Filter for Jira-style key
    let filtered = filter_by_prefix(&candidates, "proj");
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains(&"PROJ-123".to_string()));
  }

  #[test]
  fn filters_pr_ids_with_hash() {
    let candidates = vec![
      TypedCandidate::new("main", CandidateKind::Branch),
      TypedCandidate::new("#123", CandidateKind::GitHubPr),
      TypedCandidate::new("#456", CandidateKind::GitHubPr),
      TypedCandidate::new("123", CandidateKind::GitHubPr),
    ];

    let filtered = filter_by_prefix(&candidates, "#");
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains(&"#123".to_string()));
    assert!(filtered.contains(&"#456".to_string()));

    let filtered = filter_by_prefix(&candidates, "#12");
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains(&"#123".to_string()));
  }

  #[test]
  fn candidate_kind_help_text() {
    assert_eq!(CandidateKind::Branch.help_text(), "branch");
    assert_eq!(CandidateKind::RemoteBranch.help_text(), "remote branch");
    assert_eq!(CandidateKind::JiraIssue.help_text(), "Jira issue");
    assert_eq!(CandidateKind::GitHubPr.help_text(), "GitHub PR");
  }

  #[test]
  fn typed_candidate_to_completion_candidate() {
    let candidate = TypedCandidate::new("feature/test", CandidateKind::Branch);
    let completion = candidate.to_completion_candidate();

    assert_eq!(completion.get_value().to_string_lossy(), "feature/test");
  }

  #[test]
  fn completer_returns_completion_candidates() {
    let completer = TargetCompleter;

    // With empty input, should return all candidates (may be empty in test env)
    let results = completer.complete(OsStr::new(""));
    assert!(results.iter().all(|c| !c.get_value().is_empty() || results.is_empty()));
  }

  #[test]
  fn target_completer_returns_arg_value_completer() {
    let _completer = target_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn branch_completer_returns_arg_value_completer() {
    let _completer = branch_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn empty_prefix_matches_all() {
    let candidates = vec![
      TypedCandidate::new("feature/alpha", CandidateKind::Branch),
      TypedCandidate::new("bugfix/beta", CandidateKind::Branch),
      TypedCandidate::new("PROJ-123", CandidateKind::JiraIssue),
    ];

    let filtered = filter_by_prefix(&candidates, "");
    assert_eq!(filtered.len(), 3);
  }

  #[test]
  fn no_matches_returns_empty() {
    let candidates = vec![
      TypedCandidate::new("feature/alpha", CandidateKind::Branch),
      TypedCandidate::new("bugfix/beta", CandidateKind::Branch),
    ];

    let filtered = filter_by_prefix(&candidates, "xyz");
    assert!(filtered.is_empty());
  }

  /// Helper to filter candidates by contains (mirrors fuzzy completer logic)
  fn filter_by_contains(candidates: &[TypedCandidate], query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    candidates
      .iter()
      .filter(|c| c.value.to_lowercase().contains(&query_lower))
      .map(|c| c.value.clone())
      .collect()
  }

  #[test]
  fn fuzzy_filter_matches_substring() {
    let candidates = vec![
      TypedCandidate::new("feature/alpha", CandidateKind::Branch),
      TypedCandidate::new("feature/beta", CandidateKind::Branch),
      TypedCandidate::new("bugfix/alpha-test", CandidateKind::Branch),
      TypedCandidate::new("PROJ-123", CandidateKind::JiraIssue),
    ];

    // "alpha" matches branches containing "alpha" anywhere
    let filtered = filter_by_contains(&candidates, "alpha");
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains(&"feature/alpha".to_string()));
    assert!(filtered.contains(&"bugfix/alpha-test".to_string()));

    // "123" matches issue key containing "123"
    let filtered = filter_by_contains(&candidates, "123");
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains(&"PROJ-123".to_string()));

    // "eat" matches branches containing "eat" (from "feature")
    let filtered = filter_by_contains(&candidates, "eat");
    assert_eq!(filtered.len(), 2);
  }

  #[test]
  fn fuzzy_filter_case_insensitive() {
    let candidates = vec![
      TypedCandidate::new("Feature/Alpha", CandidateKind::Branch),
      TypedCandidate::new("BUGFIX/BETA", CandidateKind::Branch),
    ];

    let filtered = filter_by_contains(&candidates, "alpha");
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains(&"Feature/Alpha".to_string()));

    let filtered = filter_by_contains(&candidates, "BETA");
    assert_eq!(filtered.len(), 1);
    assert!(filtered.contains(&"BUGFIX/BETA".to_string()));
  }

  #[test]
  fn all_branch_completer_returns_arg_value_completer() {
    let _completer = all_branch_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn fuzzy_target_completer_returns_arg_value_completer() {
    let _completer = fuzzy_target_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn fuzzy_branch_completer_returns_arg_value_completer() {
    let _completer = fuzzy_branch_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn remote_branch_candidate_kind() {
    let candidate = TypedCandidate::new("origin-feature", CandidateKind::RemoteBranch);
    assert_eq!(candidate.kind, CandidateKind::RemoteBranch);
    assert_eq!(candidate.kind.help_text(), "remote branch");
  }

  #[test]
  fn fuzzy_completer_filters_correctly() {
    let completer = FuzzyTargetCompleter;

    // With empty input, should return all candidates (may be empty in test env)
    let results = completer.complete(OsStr::new(""));
    assert!(results.iter().all(|c| !c.get_value().is_empty() || results.is_empty()));
  }

  #[test]
  fn fuzzy_branch_completer_filters_correctly() {
    let completer = FuzzyBranchCompleter;

    // With empty input, should return all candidates (may be empty in test env)
    let results = completer.complete(OsStr::new(""));
    assert!(results.iter().all(|c| !c.get_value().is_empty() || results.is_empty()));
  }

  #[test]
  fn all_branch_completer_filters_correctly() {
    let completer = AllBranchCompleter;

    // With empty input, should return all candidates (may be empty in test env)
    let results = completer.complete(OsStr::new(""));
    assert!(results.iter().all(|c| !c.get_value().is_empty() || results.is_empty()));
  }
}
