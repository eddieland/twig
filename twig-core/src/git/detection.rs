//! Repository discovery helpers.

use std::env;
use std::path::{Path, PathBuf};

use git2::Repository;

/// Detect if the current directory or any parent directory is a Git repository.
pub fn detect_repository() -> Option<PathBuf> {
  let current_dir = env::current_dir().ok()?;
  detect_repository_from_path(&current_dir)
}

/// Detect if the given path or any parent directory is a Git repository.
pub fn detect_repository_from_path<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
  let path = path.as_ref();

  match Repository::discover(path) {
    Ok(repo) => repo.workdir().map(|workdir| workdir.to_path_buf()),
    Err(_) => None,
  }
}

/// Check if we're currently in a git repository.
pub fn in_git_repository() -> bool {
  detect_repository().is_some()
}

#[cfg(test)]
mod tests {
  use git2::Repository as GitRepository;
  use tempfile::TempDir;

  use super::*;

  #[test]
  fn detect_repository_none() {
    let temp_dir = TempDir::new().unwrap();
    let result = detect_repository_from_path(temp_dir.path());
    assert!(result.is_none());
  }

  #[test]
  fn detect_repository_exists() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    GitRepository::init(repo_path).unwrap();

    let maybe_result = detect_repository_from_path(repo_path);
    assert!(maybe_result.is_some());

    let result = maybe_result.unwrap();
    assert_eq!(
      std::fs::canonicalize(result).unwrap(),
      std::fs::canonicalize(repo_path).unwrap()
    );
  }

  #[test]
  fn in_git_repository_detects_current_directory() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(repo_path).unwrap();
    assert!(!in_git_repository());

    GitRepository::init(repo_path).unwrap();
    assert!(in_git_repository());

    env::set_current_dir(original_dir).unwrap();
  }
}
