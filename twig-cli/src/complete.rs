//! # Dynamic Shell Completion Support
//!
//! Provides dynamic shell completion for twig CLI commands using
//! `clap_complete::CompleteEnv`. Completions are generated at runtime from:
//! - Local Git branch names
//! - Jira issue keys associated with branches
//! - GitHub PR IDs associated with branches
//!
//! This module re-exports completers from `twig_core::complete` and provides
//! backward-compatible aliases.

// Re-export the shared completers from twig-core
pub use twig_core::complete::{
  BranchCompleter, CandidateKind, TargetCompleter, TypedCandidate, branch_completer, collect_branch_candidates,
  collect_candidates, collect_typed_candidates, target_completer,
};

/// Backward-compatible alias for `TargetCompleter`.
pub type SwitchTargetCompleter = TargetCompleter;

/// Backward-compatible alias for `target_completer()`.
pub fn switch_target_completer() -> clap_complete::engine::ArgValueCompleter {
  target_completer()
}

#[cfg(test)]
mod tests {
  use std::ffi::OsStr;

  use clap_complete::engine::ValueCompleter;

  use super::*;

  #[test]
  fn target_completer_works() {
    // Test that the completer struct works
    let completer = TargetCompleter;

    // With empty input, should return all candidates (may be empty in test env)
    let results = completer.complete(OsStr::new(""));
    assert!(results.iter().all(|c| !c.get_value().is_empty() || results.is_empty()));
  }

  #[test]
  fn switch_target_completer_function_works() {
    // Test that the factory function works
    let _completer = switch_target_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }

  #[test]
  fn branch_completer_works() {
    // Test that the branch completer works
    let _completer = branch_completer();
    // If this compiles and runs, the function is correctly returning an ArgValueCompleter
  }
}
