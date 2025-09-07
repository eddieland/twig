//#![cfg(unix)]

use std::fs;
use std::path::Path;

use anyhow::Result;
use git2::{BranchType, Repository as Git2Repository, Signature};
use twig_core::state::RepoState;
use twig_test_utils::git::GitRepoTestGuard;

/// Helper function to create a commit in a repository
fn create_commit(repo: &Git2Repository, file_name: &str, content: &str, message: &str) -> Result<()> {
  // Create a file
  let repo_path = repo.path().parent().unwrap();
  let file_path = repo_path.join(file_name);
  fs::write(&file_path, content)?;

  // Stage the file
  let mut index = repo.index()?;
  index.add_path(Path::new(file_name))?;
  index.write()?;

  // Create a commit
  let tree_id = index.write_tree()?;
  let tree = repo.find_tree(tree_id)?;

  let signature = Signature::now("Test User", "test@example.com")?;

  // Handle parent commits
  if let Ok(head) = repo.head() {
    if let Ok(parent) = head.peel_to_commit() {
      repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[&parent])?;
    } else {
      repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?;
    }
  } else {
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?;
  }

  Ok(())
}

/// Helper function to create a branch in a repository
fn create_branch(repo: &Git2Repository, branch_name: &str, start_point: Option<&str>) -> Result<()> {
  let head = if let Some(start) = start_point {
    repo
      .find_branch(start, BranchType::Local)?
      .into_reference()
      .peel_to_commit()?
  } else {
    repo.head()?.peel_to_commit()?
  };

  repo.branch(branch_name, &head, false)?;
  Ok(())
}

/// Helper function to checkout a branch
fn checkout_branch(repo: &Git2Repository, branch_name: &str) -> Result<()> {
  let obj = repo
    .revparse_single(&format!("refs/heads/{}", branch_name))?
    .peel_to_commit()?;

  repo.checkout_tree(&obj.into_object(), None)?;
  repo.set_head(&format!("refs/heads/{}", branch_name))?;

  Ok(())
}

/// Helper function to add a dependency between branches
fn add_branch_dependency(repo_path: &Path, child: &str, parent: &str) -> Result<()> {
  let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
  repo_state.add_dependency(child.to_string(), parent.to_string())?;
  repo_state.save(repo_path)?;
  Ok(())
}

/// Helper function to check if a branch exists
fn branch_exists(repo: &Git2Repository, branch_name: &str) -> bool {
  repo.find_branch(branch_name, BranchType::Local).is_ok()
}

/// Helper function to run tidy clean command
fn run_tidy_clean(repo_path: &Path, force: bool) -> Result<()> {
  use twig_cli::cli::tidy::{CleanArgs, handle_clean_command};

  let args = CleanArgs {
    dry_run: false,
    force,
  };

  // Temporarily change directory context for the test
  let current_dir = std::env::current_dir()?;
  std::env::set_current_dir(repo_path)?;
  
  let result = handle_clean_command(args);
  
  // Restore directory
  std::env::set_current_dir(current_dir)?;
  
  result
}

#[test]
fn test_tidy_cleans_simple_dependency_chain_without_changes() -> Result<()> {
  let test_guard = GitRepoTestGuard::new()?;
  let repo_path = test_guard.repo_path();
  let repo = Git2Repository::open(repo_path)?;

  // Setup: Create main branch with initial commit
  create_commit(&repo, "main.txt", "main content", "Initial commit on main")?;

  // Create branch A from main (with changes)
  create_branch(&repo, "branch-a", Some("main"))?;
  checkout_branch(&repo, "branch-a")?;
  create_commit(&repo, "a.txt", "branch a content", "Add feature A")?;

  // Create branch B from A (no additional changes)
  create_branch(&repo, "branch-b", Some("branch-a"))?;
  checkout_branch(&repo, "branch-b")?;
  // No commits on branch-b

  // Create branch C from B (no additional changes)
  create_branch(&repo, "branch-c", Some("branch-b"))?;
  checkout_branch(&repo, "branch-c")?;
  // No commits on branch-c

  // Switch back to main
  checkout_branch(&repo, "main")?;

  // Setup twig dependencies: A -> B -> C
  add_branch_dependency(repo_path, "branch-b", "branch-a")?;
  add_branch_dependency(repo_path, "branch-c", "branch-b")?;

  // Verify branches exist before cleanup
  assert!(branch_exists(&repo, "branch-a"));
  assert!(branch_exists(&repo, "branch-b"));
  assert!(branch_exists(&repo, "branch-c"));

  // Run tidy clean with force
  run_tidy_clean(repo_path, true)?;

  // Verify that B and C were cleaned up (they have no changes from their parents)
  assert!(branch_exists(&repo, "branch-a")); // Should remain (has changes)
  assert!(!branch_exists(&repo, "branch-b")); // Should be deleted (no changes)
  assert!(!branch_exists(&repo, "branch-c")); // Should be deleted (no changes)

  Ok(())
}

#[test]
fn test_tidy_preserves_chain_with_changes_in_middle() -> Result<()> {
  let test_guard = GitRepoTestGuard::new()?;
  let repo_path = test_guard.repo_path();
  let repo = Git2Repository::open(repo_path)?;

  // Setup: Create main branch with initial commit
  create_commit(&repo, "main.txt", "main content", "Initial commit on main")?;

  // Create branch A from main (no changes)
  create_branch(&repo, "branch-a", Some("main"))?;
  checkout_branch(&repo, "branch-a")?;
  // No commits on branch-a

  // Create branch B from A (with changes)
  create_branch(&repo, "branch-b", Some("branch-a"))?;
  checkout_branch(&repo, "branch-b")?;
  create_commit(&repo, "b.txt", "branch b content", "Add feature B")?;

  // Create branch C from B (no changes)
  create_branch(&repo, "branch-c", Some("branch-b"))?;
  checkout_branch(&repo, "branch-c")?;
  // No commits on branch-c

  // Switch back to main
  checkout_branch(&repo, "main")?;

  // Setup twig dependencies: A -> B -> C
  add_branch_dependency(repo_path, "branch-b", "branch-a")?;
  add_branch_dependency(repo_path, "branch-c", "branch-b")?;

  // Verify branches exist before cleanup
  assert!(branch_exists(&repo, "branch-a"));
  assert!(branch_exists(&repo, "branch-b"));
  assert!(branch_exists(&repo, "branch-c"));

  // Run tidy clean with force
  run_tidy_clean(repo_path, true)?;

  // Verify that only A and C were cleaned up appropriately
  // A should be deleted (no changes from main)
  // B should remain (has changes)  
  // C should be deleted (no changes from B)
  assert!(!branch_exists(&repo, "branch-a")); // Should be deleted (no changes from main)
  assert!(branch_exists(&repo, "branch-b"));  // Should remain (has changes)
  assert!(!branch_exists(&repo, "branch-c")); // Should be deleted (no changes from B)

  Ok(())
}

#[test]
fn test_tidy_preserves_current_branch_in_chain() -> Result<()> {
  let test_guard = GitRepoTestGuard::new()?;
  let repo_path = test_guard.repo_path();
  let repo = Git2Repository::open(repo_path)?;

  // Setup: Create main branch with initial commit
  create_commit(&repo, "main.txt", "main content", "Initial commit on main")?;

  // Create branch A from main (no changes)
  create_branch(&repo, "branch-a", Some("main"))?;
  checkout_branch(&repo, "branch-a")?;
  // No commits on branch-a

  // Create branch B from A (no changes)
  create_branch(&repo, "branch-b", Some("branch-a"))?;
  checkout_branch(&repo, "branch-b")?;
  // No commits on branch-b
  // Stay on branch-b (current branch)

  // Setup twig dependencies: A -> B
  add_branch_dependency(repo_path, "branch-b", "branch-a")?;

  // Verify branches exist before cleanup
  assert!(branch_exists(&repo, "branch-a"));
  assert!(branch_exists(&repo, "branch-b"));

  // Run tidy clean with force (should preserve current branch)
  run_tidy_clean(repo_path, true)?;

  // Verify that both branches are preserved because B is the current branch
  assert!(branch_exists(&repo, "branch-a")); // Should remain (child is current)
  assert!(branch_exists(&repo, "branch-b")); // Should remain (current branch)

  Ok(())
}
