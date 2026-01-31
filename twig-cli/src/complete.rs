//! # Dynamic Shell Completion Support
//!
//! Provides dynamic shell completion for twig CLI commands using
//! `clap_complete::CompleteEnv`. Completions are generated at runtime from:
//! - Local Git branch names
//! - Jira issue keys associated with branches
//! - GitHub PR IDs associated with branches

use std::ffi::OsStr;

use clap_complete::engine::{ArgValueCompleter, CompletionCandidate, ValueCompleter};
use twig_core::git::{get_local_branches, get_repository};
use twig_core::state::RepoState;

/// A completer that provides branch names, Jira keys, and PR IDs as candidates.
///
/// This is used for commands like `twig switch` that accept various target formats.
#[derive(Clone)]
pub struct SwitchTargetCompleter;

impl ValueCompleter for SwitchTargetCompleter {
  fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy().to_lowercase();
    collect_candidates()
      .into_iter()
      .filter(|c| c.to_lowercase().starts_with(&current_str))
      .map(CompletionCandidate::new)
      .collect()
  }
}

/// Returns an `ArgValueCompleter` for use with clap's `add` extension.
pub fn switch_target_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(SwitchTargetCompleter)
}

/// Collect all completion candidates from available data sources.
fn collect_candidates() -> Vec<String> {
  let mut candidates = Vec::new();

  // Collect branch names
  if let Ok(branches) = get_local_branches() {
    candidates.extend(branches);
  }

  // Collect Jira keys and PR IDs from repo state
  if let Some(repo) = get_repository()
    && let Some(workdir) = repo.workdir()
    && let Ok(state) = RepoState::load(workdir)
  {
    // Add Jira issue keys
    for jira_key in state.jira_to_branch_index.keys() {
      candidates.push(jira_key.clone());
    }

    // Add GitHub PR IDs (prefixed with # for clarity)
    for metadata in state.branches.values() {
      if let Some(pr_id) = metadata.github_pr {
        candidates.push(format!("#{pr_id}"));
        // Also add plain number for convenience
        candidates.push(pr_id.to_string());
      }
    }
  }

  // Remove duplicates while preserving order
  let mut seen = std::collections::HashSet::new();
  candidates.retain(|c| seen.insert(c.clone()));

  // Sort for consistent output
  candidates.sort();

  candidates
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Helper to filter candidates by prefix (mirrors the completer logic)
  fn filter_by_prefix(candidates: &[String], prefix: &str) -> Vec<String> {
    let prefix_lower = prefix.to_lowercase();
    candidates
      .iter()
      .filter(|c| c.to_lowercase().starts_with(&prefix_lower))
      .cloned()
      .collect()
  }

  /// Helper to deduplicate candidates (mirrors the collect_candidates logic)
  fn deduplicate(mut candidates: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.clone()));
    candidates.sort();
    candidates
  }

  #[test]
  fn filters_by_prefix_case_insensitive() {
    let candidates = vec![
      "feature/alpha".to_string(),
      "feature/beta".to_string(),
      "Feature/gamma".to_string(),
      "bugfix/delta".to_string(),
      "PROJ-123".to_string(),
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
      "main".to_string(),
      "#123".to_string(),
      "#456".to_string(),
      "123".to_string(),
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
  fn deduplicates_candidates() {
    let candidates = vec![
      "feature/test".to_string(),
      "main".to_string(),
      "feature/test".to_string(), // duplicate
      "develop".to_string(),
      "main".to_string(), // duplicate
    ];

    let deduped = deduplicate(candidates);
    assert_eq!(deduped.len(), 3);

    // Count occurrences
    let main_count = deduped.iter().filter(|c| *c == "main").count();
    assert_eq!(main_count, 1);

    let feature_count = deduped.iter().filter(|c| *c == "feature/test").count();
    assert_eq!(feature_count, 1);
  }

  #[test]
  fn completer_returns_completion_candidates() {
    // Test that the completer struct can be instantiated and used
    let completer = SwitchTargetCompleter;

    // With empty input, should return all candidates (may be empty in test env)
    let results = completer.complete(OsStr::new(""));
    // We can't assert on specific values since we don't control the test environment,
    // but we can verify it returns a Vec<CompletionCandidate>
    assert!(results.iter().all(|c| !c.get_value().is_empty() || results.is_empty()));
  }

  #[test]
  fn switch_target_completer_returns_arg_value_completer() {
    // Test that the factory function works
    let _completer = switch_target_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn empty_prefix_matches_all() {
    let candidates = vec![
      "feature/alpha".to_string(),
      "bugfix/beta".to_string(),
      "PROJ-123".to_string(),
    ];

    let filtered = filter_by_prefix(&candidates, "");
    assert_eq!(filtered.len(), 3);
  }

  #[test]
  fn no_matches_returns_empty() {
    let candidates = vec!["feature/alpha".to_string(), "bugfix/beta".to_string()];

    let filtered = filter_by_prefix(&candidates, "xyz");
    assert!(filtered.is_empty());
  }
}
