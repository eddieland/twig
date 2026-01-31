//! # Shell Completion Support
//!
//! Provides dynamic shell completion for the `twig flow` plugin using
//! `clap_complete::CompleteEnv`. Completions are generated at runtime from:
//! - Local Git branch names
//! - Jira issue keys associated with branches
//! - GitHub PR IDs associated with branches
//!
//! This module re-exports completers from `twig_core::complete`.

pub use twig_core::complete::target_completer;

/// Backward-compatible alias for `target_completer()`.
pub fn flow_target_completer() -> clap_complete::engine::ArgValueCompleter {
  target_completer()
}

#[cfg(test)]
mod tests {
  use std::ffi::OsStr;

  use clap_complete::engine::ValueCompleter;
  use twig_core::complete::{TargetCompleter, collect_candidates};
  use twig_core::state::{BranchMetadata, RepoState};
  use twig_test_utils::GitRepoTestGuard;

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

    let completer = TargetCompleter;
    let results = completer.complete(OsStr::new("feat"));

    let values: Vec<String> = results
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
