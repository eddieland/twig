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

use crate::git::{get_local_branches, get_repository};
use crate::state::RepoState;

/// The type of completion candidate, used for help text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateKind {
  /// A local Git branch
  Branch,
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
  /// Pre-computed lowercase value for case-insensitive matching.
  value_lower: String,
  /// The type of candidate.
  pub kind: CandidateKind,
}

impl TypedCandidate {
  /// Create a new typed candidate.
  pub fn new(value: impl Into<String>, kind: CandidateKind) -> Self {
    let value = value.into();
    let value_lower = value.to_lowercase();
    Self {
      value,
      value_lower,
      kind,
    }
  }

  /// Returns whether this candidate's value starts with the given lowercase prefix.
  pub fn matches_prefix(&self, prefix_lower: &str) -> bool {
    self.value_lower.starts_with(prefix_lower)
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

  // Collect local branch names into a set for cross-referencing
  let local_branches: HashSet<String> = get_local_branches().unwrap_or_default().into_iter().collect();

  for branch in &local_branches {
    if seen.insert(branch.clone()) {
      candidates.push(TypedCandidate::new(branch.clone(), CandidateKind::Branch));
    }
  }

  // Collect Jira keys and PR IDs from repo state, but only for branches
  // that still exist locally. The state accumulates entries over time and
  // stale associations would overwhelm the completion list.
  if let Some(repo) = get_repository()
    && let Some(workdir) = repo.workdir()
    && let Ok(state) = RepoState::load(workdir)
  {
    // Add Jira issue keys only when the associated branch exists locally
    for (jira_key, branch_name) in &state.jira_to_branch_index {
      if local_branches.contains(branch_name) && seen.insert(jira_key.clone()) {
        candidates.push(TypedCandidate::new(jira_key.clone(), CandidateKind::JiraIssue));
      }
    }

    // Add GitHub PR IDs only when the associated branch exists locally
    for (branch_name, metadata) in &state.branches {
      if local_branches.contains(branch_name)
        && let Some(pr_id) = metadata.github_pr
      {
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

/// Collect only branch name candidates.
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
      .filter(|c| c.matches_prefix(&current_str))
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
      .filter(|c| c.matches_prefix(&current_str))
      .map(|c| c.to_completion_candidate())
      .collect()
  }
}

/// Returns an `ArgValueCompleter` that only provides branch names.
pub fn branch_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(BranchCompleter)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Helper to filter candidates by prefix (mirrors the completer logic)
  fn filter_by_prefix(candidates: &[TypedCandidate], prefix: &str) -> Vec<String> {
    let prefix_lower = prefix.to_lowercase();
    candidates
      .iter()
      .filter(|c| c.matches_prefix(&prefix_lower))
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
}
