//! # Utility Functions
//!
//! Common utility functions for path manipulation, string processing,
//! and other helper functions used throughout the twig ecosystem.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Resolve a repository path to its canonical form.
///
/// Normalizes both absolute and relative paths so callers can reliably
/// compare repository locations even when invoked from different working
/// directories.
///
/// # Arguments
///
/// * `path` - A filesystem path that may be absolute or relative to the current
///   working directory.
///
/// # Errors
///
/// Returns an error when the path does not exist or cannot be canonicalized
/// by the operating system. The error context includes the display form of
/// the provided path for easier debugging.
pub fn resolve_repository_path<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
  let path = path.as_ref();

  // If it's already absolute, canonicalize it
  if path.is_absolute() {
    return std::fs::canonicalize(path).with_context(|| format!("Failed to resolve path: {}", path.display()));
  }

  // If it's relative, resolve it relative to current directory
  let current_dir = std::env::current_dir().context("Failed to get current directory")?;

  let full_path = current_dir.join(path);
  std::fs::canonicalize(full_path).with_context(|| format!("Failed to resolve path: {}", path.display()))
}

/// Normalize a path for consistent display.
///
/// Expands the path into a user-friendly string, preferring `~/` prefixes
/// when a path resides under the current user's home directory. This keeps
/// CLI output concise while still pointing at the exact location.
///
/// # Arguments
///
/// * `path` - Any filesystem path to display back to the user.
pub fn normalize_path_display<P: AsRef<Path>>(path: P) -> String {
  let path = path.as_ref();

  // Try to make it relative to home directory if possible
  if let Ok(home_dir) = std::env::var("HOME") {
    let home_path = PathBuf::from(home_dir);
    if let Ok(relative) = path.strip_prefix(&home_path) {
      return format!("~/{}", relative.display());
    }
  }

  path.display().to_string()
}

/// Check if a string is a valid branch name.
///
/// Applies a subset of Git's branch validation rules to catch common
/// mistakes before attempting to create or checkout a branch.
///
/// # Arguments
///
/// * `name` - The branch candidate to validate.
///
/// # Returns
///
/// `true` when the string is acceptable and `false` otherwise.
pub fn is_valid_branch_name(name: &str) -> bool {
  if name.is_empty() {
    return false;
  }

  // Basic Git branch name validation
  // Cannot start or end with slash, cannot contain consecutive slashes
  if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
    return false;
  }

  // Cannot contain certain characters
  let invalid_chars = [' ', '~', '^', ':', '?', '*', '[', '\\'];
  if name.chars().any(|c| invalid_chars.contains(&c)) {
    return false;
  }

  // Cannot be just dots
  if name.chars().all(|c| c == '.') {
    return false;
  }

  // Cannot contain @{
  if name.contains("@{") {
    return false;
  }

  true
}

/// Truncate a string to a maximum length with ellipsis.
///
/// Preserves the start of the string and appends an ellipsis so the caller
/// can display bounded-length content without losing context entirely.
///
/// # Arguments
///
/// * `s` - Source string to shorten.
/// * `max_len` - Maximum number of characters to return, including the ellipsis
///   when truncation occurs.
pub fn truncate_string(s: &str, max_len: usize) -> String {
  if s.len() <= max_len {
    s.to_string()
  } else if max_len <= 3 {
    "...".to_string()
  } else {
    format!("{}...", &s[..max_len - 3])
  }
}

/// Convert a duration in seconds to a human-readable format.
///
/// The output uses the largest appropriate unit (`s`, `m`, `h`, `d`) and
/// combines units when needed for better readability.
///
/// # Arguments
///
/// * `seconds` - Length of time expressed in whole seconds.
pub fn format_duration(seconds: u64) -> String {
  if seconds < 60 {
    format!("{seconds}s")
  } else if seconds < 3600 {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    if remaining_seconds == 0 {
      format!("{minutes}m")
    } else {
      format!("{minutes}m {remaining_seconds}s")
    }
  } else if seconds < 86400 {
    let hours = seconds / 3600;
    let remaining_minutes = (seconds % 3600) / 60;
    if remaining_minutes == 0 {
      format!("{hours}h")
    } else {
      format!("{hours}h {remaining_minutes}m")
    }
  } else {
    let days = seconds / 86400;
    let remaining_hours = (seconds % 86400) / 3600;
    if remaining_hours == 0 {
      format!("{days}d")
    } else {
      format!("{days}d {remaining_hours}h")
    }
  }
}

/// Extract the repository name from a path.
///
/// Returns the final path component or "unknown" when the path cannot be
/// represented as UTF-8 (which should be exceedingly rare on supported
/// platforms).
///
/// # Arguments
///
/// * `path` - Path pointing anywhere inside or at the root of a repository.
pub fn extract_repo_name<P: AsRef<Path>>(path: P) -> String {
  path
    .as_ref()
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("unknown")
    .to_string()
}

/// Get the Jira issue associated with the current branch.
///
/// Looks up the active Git branch inside the current repository and returns
/// the Jira issue key that twig previously associated with the branch via the
/// repository state file.
///
/// # Errors
///
/// Returns an error when not inside a Git repository, when the repository has
/// no active branch (detached HEAD), or when the state file cannot be loaded.
pub fn get_current_branch_jira_issue() -> Result<Option<String>> {
  use crate::git::{current_branch, detect_repository};
  use crate::state::RepoState;

  // Get the current repository path
  let repo_path = detect_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  // Get the current branch name
  let branch_name = current_branch()?.ok_or_else(|| anyhow::anyhow!("Not on any branch"))?;

  // Load the repository state
  let state = RepoState::load(&repo_path)?;

  // Get the branch metadata and return the Jira issue
  Ok(
    state
      .get_branch_metadata(&branch_name)
      .and_then(|metadata| metadata.jira_issue.clone()),
  )
}

/// Get the GitHub PR number associated with the current branch.
///
/// Reads repository metadata maintained by twig to determine whether the
/// current branch is linked to a GitHub pull request.
///
/// # Errors
///
/// Returns an error when the repository cannot be detected, the active branch
/// cannot be determined, or the state file fails to load.
pub fn get_current_branch_github_pr() -> Result<Option<u32>> {
  use crate::git::{current_branch, detect_repository};
  use crate::state::RepoState;

  // Get the current repository path
  let repo_path = detect_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  // Get the current branch name
  let branch_name = current_branch()?.ok_or_else(|| anyhow::anyhow!("Not on any branch"))?;

  // Load the repository state
  let state = RepoState::load(&repo_path)?;

  // Get the branch metadata and return the GitHub PR number
  Ok(
    state
      .get_branch_metadata(&branch_name)
      .and_then(|metadata| metadata.github_pr),
  )
}

/// Open a URL in the default browser
pub fn open_url_in_browser(url: &str) -> Result<()> {
  use crate::output::{print_success, print_warning};

  match open::that(url) {
    Ok(()) => {
      print_success(&format!("Opening {url} in browser..."));
      Ok(())
    }
    Err(e) => {
      print_warning(&format!("Failed to open browser to {url}: {e}"));
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_resolve_repository_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    let resolved = resolve_repository_path(&path).unwrap();
    assert_eq!(resolved, std::fs::canonicalize(path).unwrap());
  }

  #[test]
  fn test_is_valid_branch_name() {
    assert!(is_valid_branch_name("main"));
    assert!(is_valid_branch_name("feature/new-feature"));
    assert!(is_valid_branch_name("bugfix-123"));

    assert!(!is_valid_branch_name(""));
    assert!(!is_valid_branch_name("/invalid"));
    assert!(!is_valid_branch_name("invalid/"));
    assert!(!is_valid_branch_name("invalid//name"));
    assert!(!is_valid_branch_name("invalid name"));
    assert!(!is_valid_branch_name("invalid~name"));
    assert!(!is_valid_branch_name("..."));
    assert!(!is_valid_branch_name("invalid@{name"));
  }

  #[test]
  fn test_truncate_string() {
    assert_eq!(truncate_string("hello", 10), "hello");
    assert_eq!(truncate_string("hello world", 8), "hello...");
    assert_eq!(truncate_string("hi", 2), "hi");
    assert_eq!(truncate_string("hello", 3), "...");
  }

  #[test]
  fn test_format_duration() {
    assert_eq!(format_duration(30), "30s");
    assert_eq!(format_duration(60), "1m");
    assert_eq!(format_duration(90), "1m 30s");
    assert_eq!(format_duration(3600), "1h");
    assert_eq!(format_duration(3660), "1h 1m");
    assert_eq!(format_duration(86400), "1d");
    assert_eq!(format_duration(90000), "1d 1h");
  }

  #[test]
  fn test_extract_repo_name() {
    assert_eq!(extract_repo_name("/path/to/my-repo"), "my-repo");
    assert_eq!(extract_repo_name("my-repo"), "my-repo");
    assert_eq!(extract_repo_name("/"), "unknown");
  }
}
