//! Helpers for opening Git repositories.

use std::path::Path;

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
