//! # Shell Completion Support
//!
//! Provides dynamic shell completion for the `twig flow` plugin using
//! `clap_complete::CompleteEnv`. Completions are generated at runtime from:
//! - Local Git branch names
//! - Jira issue keys associated with branches
//! - GitHub PR IDs associated with branches

use std::ffi::OsStr;

use clap_complete::engine::{ArgValueCompleter, CompletionCandidate, ValueCompleter};
use twig_core::git::{get_local_branches, get_repository};
use twig_core::state::RepoState;

/// A completer that provides branch names, Jira keys, and PR IDs as candidates.
#[derive(Clone)]
pub struct FlowTargetCompleter;

impl ValueCompleter for FlowTargetCompleter {
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
pub fn flow_target_completer() -> ArgValueCompleter {
  ArgValueCompleter::new(FlowTargetCompleter)
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
  use twig_core::state::{BranchMetadata, RepoState};
  use twig_test_utils::GitRepoTestGuard;

  use super::*;

  #[test]
  fn collects_branch_names() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    twig_test_utils::create_commit(&guard.repo, "file.txt", "content", "initial").unwrap();
    twig_test_utils::create_branch(&guard.repo, "feature/test", None).unwrap();
    twig_test_utils::create_branch(&guard.repo, "bugfix/issue", None).unwrap();

    let candidates = collect_candidates();

    assert!(candidates.contains(&"feature/test".to_string()));
    assert!(candidates.contains(&"bugfix/issue".to_string()));
  }

  #[test]
  fn collects_jira_keys_from_state() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    twig_test_utils::create_commit(&guard.repo, "file.txt", "content", "initial").unwrap();

    let repo_path = guard.repo.workdir().unwrap();
    let mut state = RepoState::load(repo_path).unwrap();
    state.add_branch_issue(BranchMetadata {
      branch: "feature/work".into(),
      jira_issue: Some("PROJ-123".into()),
      github_pr: None,
      created_at: "now".into(),
    });
    state.save(repo_path).unwrap();

    // Reload to rebuild indices
    let _state = RepoState::load(repo_path).unwrap();

    let candidates = collect_candidates();
    assert!(candidates.contains(&"PROJ-123".to_string()));
  }

  #[test]
  fn collects_github_pr_ids_from_state() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    twig_test_utils::create_commit(&guard.repo, "file.txt", "content", "initial").unwrap();

    let repo_path = guard.repo.workdir().unwrap();
    let mut state = RepoState::load(repo_path).unwrap();
    state.add_branch_issue(BranchMetadata {
      branch: "feature/pr".into(),
      jira_issue: None,
      github_pr: Some(456),
      created_at: "now".into(),
    });
    state.save(repo_path).unwrap();

    let candidates = collect_candidates();
    assert!(candidates.contains(&"#456".to_string()));
    assert!(candidates.contains(&"456".to_string()));
  }

  #[test]
  fn completer_filters_by_prefix() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    twig_test_utils::create_commit(&guard.repo, "file.txt", "content", "initial").unwrap();
    twig_test_utils::create_branch(&guard.repo, "feature/alpha", None).unwrap();
    twig_test_utils::create_branch(&guard.repo, "feature/beta", None).unwrap();
    twig_test_utils::create_branch(&guard.repo, "bugfix/gamma", None).unwrap();

    let completer = FlowTargetCompleter;
    let results = completer.complete(OsStr::new("feat"));

    let values: Vec<_> = results
      .iter()
      .map(|c| c.get_value().to_string_lossy().to_string())
      .collect();
    assert!(values.iter().any(|v| v.contains("alpha")));
    assert!(values.iter().any(|v| v.contains("beta")));
    assert!(!values.iter().any(|v| v.contains("gamma")));
  }

  #[test]
  fn removes_duplicates() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    twig_test_utils::create_commit(&guard.repo, "file.txt", "content", "initial").unwrap();

    // Create a branch that will appear in both git branches and state
    twig_test_utils::create_branch(&guard.repo, "feature/dup", None).unwrap();

    let repo_path = guard.repo.workdir().unwrap();
    let mut state = RepoState::load(repo_path).unwrap();
    state.add_branch_issue(BranchMetadata {
      branch: "feature/dup".into(),
      jira_issue: Some("DUP-1".into()),
      github_pr: None,
      created_at: "now".into(),
    });
    state.save(repo_path).unwrap();

    let candidates = collect_candidates();

    // Count occurrences of "feature/dup"
    let count = candidates.iter().filter(|c| *c == "feature/dup").count();
    assert_eq!(count, 1, "Branch name should appear only once");
  }
}
