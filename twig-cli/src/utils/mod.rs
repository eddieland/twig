//! # Utility Functions
//!
//! Common utility functions and helpers for file operations, Git repository
//! validation, and shared functionality across the twig application.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub mod output;

/// Check if a path is a valid git repository
pub fn is_git_repository<P: AsRef<Path>>(path: P) -> bool {
  let git_dir = path.as_ref().join(".git");
  git_dir.exists() && git_dir.is_dir()
}

/// Resolve a repository path from a command line argument or current directory
pub fn resolve_repository_path(repo_arg: Option<&str>) -> Result<PathBuf> {
  match repo_arg {
    Some(path) => {
      let path_buf = PathBuf::from(path);
      if !path_buf.exists() {
        return Err(anyhow::anyhow!("Repository path does not exist: {}", path));
      }
      if !is_git_repository(&path_buf) {
        return Err(anyhow::anyhow!("Not a git repository: {}", path));
      }
      Ok(path_buf)
    }
    None => {
      // Try to detect the current repository
      crate::git::detect_current_repository().context("No repository specified and not in a git repository")
    }
  }
}
