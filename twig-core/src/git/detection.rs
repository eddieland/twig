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

/// Resolve a path to the main (non-worktree) repository working directory.
///
/// When called from inside a git worktree, this returns the path to the main
/// repository rather than the worktree itself. For regular (non-worktree)
/// repositories, this behaves identically to [`detect_repository_from_path`].
///
/// This is useful for the global registry, which should track the main
/// repository rather than individual worktrees — so that worktrees of the same
/// repo are not counted as separate repositories.
pub fn resolve_to_main_repo_path<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
  let path = path.as_ref();

  let repo = Repository::discover(path).ok()?;

  let raw = if repo.is_worktree() {
    // For linked worktrees, commondir() points to the main repo's .git
    // directory. Its parent is the main repo's working directory.
    repo.commondir().parent().map(|p| p.to_path_buf())?
  } else {
    repo.workdir().map(|workdir| workdir.to_path_buf())?
  };

  // Canonicalize so the result matches paths stored via fs::canonicalize
  // elsewhere (e.g. Registry). On Windows, canonicalize adds the \\?\ prefix
  // that raw git2 paths lack.
  std::fs::canonicalize(raw).ok()
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

  /// Helper: create a repo with an initial commit and a named branch, then
  /// create a linked worktree for that branch. Returns `(main_path, worktree_path)`.
  fn setup_repo_with_worktree(temp_dir: &TempDir) -> (PathBuf, PathBuf) {
    let main_path = temp_dir.path().join("main-repo");
    std::fs::create_dir_all(&main_path).unwrap();

    let repo = GitRepository::init(&main_path).unwrap();
    let sig = git2::Signature::now("Test", "test@test.com").unwrap();
    let tree_id = repo.index().unwrap().write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    repo
      .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
      .unwrap();

    let head = repo.head().unwrap();
    let commit = head.peel_to_commit().unwrap();
    repo.branch("wt-branch", &commit, false).unwrap();

    let wt_path = temp_dir.path().join("my-worktree");
    repo.worktree("my-worktree", &wt_path, None).unwrap();

    (main_path, wt_path)
  }

  #[test]
  fn resolve_to_main_repo_path_returns_main_for_worktree() {
    let temp_dir = TempDir::new().unwrap();
    let (main_path, wt_path) = setup_repo_with_worktree(&temp_dir);

    let resolved = resolve_to_main_repo_path(&wt_path);
    assert!(resolved.is_some(), "should resolve worktree to main repo");

    let resolved_canonical = std::fs::canonicalize(resolved.unwrap()).unwrap();
    let main_canonical = std::fs::canonicalize(&main_path).unwrap();
    assert_eq!(
      resolved_canonical, main_canonical,
      "resolve_to_main_repo_path should return the main repo path, not the worktree"
    );
  }

  #[test]
  fn resolve_to_main_repo_path_returns_self_for_regular_repo() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    GitRepository::init(repo_path).unwrap();

    let resolved = resolve_to_main_repo_path(repo_path);
    assert!(resolved.is_some());

    let resolved_canonical = std::fs::canonicalize(resolved.unwrap()).unwrap();
    let repo_canonical = std::fs::canonicalize(repo_path).unwrap();
    assert_eq!(resolved_canonical, repo_canonical);
  }

  #[test]
  fn resolve_to_main_repo_path_returns_none_for_non_repo() {
    let temp_dir = TempDir::new().unwrap();
    let result = resolve_to_main_repo_path(temp_dir.path());
    assert!(result.is_none());
  }

  #[test]
  fn detect_repository_from_path_returns_worktree_path() {
    let temp_dir = TempDir::new().unwrap();
    let (_main_path, wt_path) = setup_repo_with_worktree(&temp_dir);

    // detect_repository_from_path should return the worktree's own path
    // (unlike resolve_to_main_repo_path which returns the main repo)
    let detected = detect_repository_from_path(&wt_path);
    assert!(detected.is_some());

    let detected_canonical = std::fs::canonicalize(detected.unwrap()).unwrap();
    let wt_canonical = std::fs::canonicalize(&wt_path).unwrap();
    assert_eq!(
      detected_canonical, wt_canonical,
      "detect_repository_from_path should return the worktree path"
    );
  }
}
