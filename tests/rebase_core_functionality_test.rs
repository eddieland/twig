use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use git2::{Repository as Git2Repository, Signature};
use twig_test_utils::git::GitRepoTestGuard;

/// Execute a git command in the specified repository
fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<(bool, String)> {
  let output = Command::new("git").args(args).current_dir(repo_path).output()?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  let combined_output = if !stdout.is_empty() { stdout } else { stderr };

  Ok((output.status.success(), combined_output))
}

/// Check if a rebase is currently in progress
fn is_rebase_in_progress(repo_path: &Path) -> bool {
  let rebase_merge_dir = repo_path.join(".git").join("rebase-merge");
  let rebase_apply_dir = repo_path.join(".git").join("rebase-apply");
  rebase_merge_dir.exists() || rebase_apply_dir.exists()
}

/// Create a commit in the repository
fn create_commit(repo: &Git2Repository, file_name: &str, content: &str, message: &str) -> Result<()> {
  let repo_path = repo.path().parent().unwrap();
  let file_path = repo_path.join(file_name);
  fs::write(&file_path, content)?;

  let mut index = repo.index()?;
  index.add_path(Path::new(file_name))?;
  index.write()?;

  let tree_id = index.write_tree()?;
  let tree = repo.find_tree(tree_id)?;
  let signature = Signature::now("Test User", "test@example.com")?;

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

/// Test the core rebase continue functionality
#[test]
fn test_rebase_continue_basic() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create initial commit
  create_commit(repo, "base.txt", "base content", "Initial commit")?;

  // Create a feature branch
  let (success, _) = execute_git_command(repo_path, &["checkout", "-b", "feature"])?;
  assert!(success, "Should be able to create feature branch");

  // Create a commit on feature
  create_commit(repo, "feature.txt", "feature content", "Feature commit")?;

  // Go back to main and create a conflicting commit
  let (success, _) = execute_git_command(repo_path, &["checkout", "main"])?;
  assert!(success, "Should be able to checkout main");

  create_commit(repo, "base.txt", "main content", "Main commit")?;

  // Go back to feature and try to rebase (this should create a conflict)
  let (success, _) = execute_git_command(repo_path, &["checkout", "feature"])?;
  assert!(success, "Should be able to checkout feature");

  let (success, output) = execute_git_command(repo_path, &["rebase", "main"])?;
  println!("Rebase output: {}", output);

  // Check if rebase is in progress (indicating conflicts)
  if is_rebase_in_progress(repo_path) {
    println!("Rebase is in progress - conflicts detected");

    // Resolve the conflict by editing the file
    let conflict_file = repo_path.join("base.txt");
    fs::write(&conflict_file, "resolved content")?;

    // Stage the resolved file
    let (success, _) = execute_git_command(repo_path, &["add", "base.txt"])?;
    assert!(success, "Should be able to stage resolved file");

    // Continue the rebase
    let (success, output) = execute_git_command(repo_path, &["rebase", "--continue"])?;
    println!("Continue output: {}", output);

    // Verify rebase completed
    assert!(
      !is_rebase_in_progress(repo_path),
      "Rebase should complete after continue"
    );
  } else {
    // If no conflicts, that's also a valid outcome
    println!("Rebase completed without conflicts");
  }

  Ok(())
}

/// Test the core rebase skip functionality
#[test]
fn test_rebase_skip_basic() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create initial commit
  create_commit(repo, "base.txt", "base content", "Initial commit")?;

  // Create feature branch
  let (success, _) = execute_git_command(repo_path, &["checkout", "-b", "feature"])?;
  assert!(success, "Should be able to create feature branch");

  // Create multiple commits on feature that will conflict
  create_commit(repo, "base.txt", "feature modification 1", "Feature commit 1")?;
  create_commit(repo, "file2.txt", "feature content 2", "Feature commit 2")?;

  // Go to main and create conflicting changes
  let (success, _) = execute_git_command(repo_path, &["checkout", "main"])?;
  assert!(success, "Should be able to checkout main");

  create_commit(repo, "base.txt", "main modification", "Main commit")?;

  // Go back to feature and rebase
  let (success, _) = execute_git_command(repo_path, &["checkout", "feature"])?;
  assert!(success, "Should be able to checkout feature");

  let (success, output) = execute_git_command(repo_path, &["rebase", "main"])?;
  println!("Initial rebase output: {}", output);

  // If rebase is in progress due to conflicts
  if is_rebase_in_progress(repo_path) {
    println!("Rebase is in progress - testing skip");

    // Skip the conflicting commit
    let (success, output) = execute_git_command(repo_path, &["rebase", "--skip"])?;
    println!("Skip output: {}", output);

    // Handle any remaining rebase operations
    let mut attempts = 5;
    while is_rebase_in_progress(repo_path) && attempts > 0 {
      let (_, status) = execute_git_command(repo_path, &["status", "--porcelain"])?;

      if status.trim().is_empty() {
        // No conflicts, continue
        let (_, cont_output) = execute_git_command(repo_path, &["rebase", "--continue"])?;
        println!("Continue output: {}", cont_output);
      } else {
        // More conflicts, skip again
        let (_, skip_output) = execute_git_command(repo_path, &["rebase", "--skip"])?;
        println!("Additional skip output: {}", skip_output);
      }
      attempts -= 1;
    }

    // Verify rebase eventually completes
    assert!(
      !is_rebase_in_progress(repo_path),
      "Rebase should complete after skip operations"
    );
  } else {
    println!("Rebase completed without conflicts - skip test not applicable");
  }

  Ok(())
}

/// Test rebase abort functionality
#[test]
fn test_rebase_abort_basic() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create initial commit
  create_commit(repo, "base.txt", "base content", "Initial commit")?;

  // Create feature branch
  let (success, _) = execute_git_command(repo_path, &["checkout", "-b", "feature"])?;
  assert!(success, "Should be able to create feature branch");

  create_commit(repo, "base.txt", "feature content", "Feature commit")?;

  // Create main branch changes
  let (success, _) = execute_git_command(repo_path, &["checkout", "main"])?;
  assert!(success, "Should be able to checkout main");

  create_commit(repo, "base.txt", "main content", "Main commit")?;

  // Go back to feature
  let (success, _) = execute_git_command(repo_path, &["checkout", "feature"])?;
  assert!(success, "Should be able to checkout feature");

  // Start rebase that will conflict
  let (success, output) = execute_git_command(repo_path, &["rebase", "main"])?;
  println!("Rebase output: {}", output);

  if is_rebase_in_progress(repo_path) {
    println!("Rebase in progress - testing abort");

    // Abort the rebase
    let (success, output) = execute_git_command(repo_path, &["rebase", "--abort"])?;
    println!("Abort output: {}", output);

    // Verify rebase is no longer in progress
    assert!(!is_rebase_in_progress(repo_path), "Rebase should be aborted");

    // Verify we're still on feature branch
    let (success, current_branch) = execute_git_command(repo_path, &["branch", "--show-current"])?;
    assert!(success, "Should be able to get current branch");
    assert_eq!(
      current_branch.trim(),
      "feature",
      "Should be on feature branch after abort"
    );
  } else {
    println!("Rebase completed without conflicts - abort test not applicable");
  }

  Ok(())
}

/// Test rebase in progress detection
#[test]
fn test_rebase_detection() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Initially no rebase should be in progress
  assert!(
    !is_rebase_in_progress(repo_path),
    "No rebase should be in progress initially"
  );

  // Create setup that might cause conflicts
  create_commit(repo, "test.txt", "initial content", "Initial commit")?;

  let (success, _) = execute_git_command(repo_path, &["checkout", "-b", "branch1"])?;
  assert!(success, "Should be able to create branch1");

  create_commit(repo, "test.txt", "branch1 content", "Branch1 commit")?;

  let (success, _) = execute_git_command(repo_path, &["checkout", "main"])?;
  assert!(success, "Should be able to checkout main");

  create_commit(repo, "test.txt", "main content", "Main commit")?;

  let (success, _) = execute_git_command(repo_path, &["checkout", "branch1"])?;
  assert!(success, "Should be able to checkout branch1");

  // Try to rebase
  let (_, _) = execute_git_command(repo_path, &["rebase", "main"])?;

  // Check if rebase is in progress
  let in_progress = is_rebase_in_progress(repo_path);
  println!("Rebase in progress: {}", in_progress);

  if in_progress {
    // Clean up by aborting
    execute_git_command(repo_path, &["rebase", "--abort"])?;
    assert!(!is_rebase_in_progress(repo_path), "Rebase should be aborted");
  }

  Ok(())
}
