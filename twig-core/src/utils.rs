//! # Utility Functions
//!
//! Common utility functions for path manipulation, string processing,
//! and other helper functions used throughout the twig ecosystem.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Resolve a repository path to its canonical form
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

/// Normalize a path for consistent display
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

/// Check if a string is a valid branch name
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

/// Truncate a string to a maximum length with ellipsis
pub fn truncate_string(s: &str, max_len: usize) -> String {
  if s.len() <= max_len {
    s.to_string()
  } else if max_len <= 3 {
    "...".to_string()
  } else {
    format!("{}...", &s[..max_len - 3])
  }
}

/// Convert a duration in seconds to a human-readable format
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

/// Extract the repository name from a path
pub fn extract_repo_name<P: AsRef<Path>>(path: P) -> String {
  path
    .as_ref()
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("unknown")
    .to_string()
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_resolve_repository_path() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    let resolved = resolve_repository_path(path).unwrap();
    assert_eq!(resolved, path);
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
