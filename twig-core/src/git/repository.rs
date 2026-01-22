//! Helpers for opening Git repositories.

use std::path::{Path, PathBuf};

use anyhow::Context;
use git2::Repository;

use super::detection::{detect_repository, detect_repository_from_path};

/// Get the Git repository object for the current directory.
pub fn get_repository() -> Option<Repository> {
  let repo_path = detect_repository()?;

  let repo = Repository::open(&repo_path)
    .context("Failed to open Git repository")
    .ok()?;

  Some(repo)
}

/// Get the Git repository object for a specific path.
pub fn get_repository_from_path<P: AsRef<Path>>(path: P) -> Option<Repository> {
  let repo_path = detect_repository_from_path(path)?;

  let repo = Repository::open(&repo_path)
    .context("Failed to open Git repository")
    .ok()?;

  Some(repo)
}

/// Resolve the primary repository workdir for storing shared metadata.
///
/// Git worktrees have their own working directories, but share a common
/// repository. This helper returns the main workdir so per-repository state
/// is shared across worktrees.
pub fn resolve_repository_root<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
  let repo = Repository::discover(path).ok()?;
  let common_dir = repo.commondir().ok()?;

  let repo_for_state = if common_dir != repo.path() {
    Repository::open(common_dir).ok()?
  } else {
    repo
  };

  repo_for_state.workdir().map(|workdir| workdir.to_path_buf())
}
