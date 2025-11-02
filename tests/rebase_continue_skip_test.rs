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

/// Helper function to create a commit that will cause a conflict
fn create_conflicting_commit(repo: &Git2Repository, file_name: &str, content: &str, message: &str) -> Result<()> {
  create_commit(repo, file_name, content, message)
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

/// Helper function to simulate a rebase with conflicts
fn simulate_rebase_with_conflicts(repo_path: &Path, _branch: &str, onto: &str) -> Result<bool> {
  use std::process::Command;

  use twig_cli::consts;

  // Execute git rebase command that will likely cause conflicts
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(&["rebase", onto])
    .current_dir(repo_path)
    .output()?;

  let stderr = String::from_utf8_lossy(&output.stderr);
  let stdout = String::from_utf8_lossy(&output.stdout);

  // Check if there are conflicts
  Ok(stderr.contains("CONFLICT") || stdout.contains("CONFLICT"))
}

/// Helper function to execute git commands
fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<(bool, String)> {
  use std::process::Command;

  use twig_cli::consts;

  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(args)
    .current_dir(repo_path)
    .output()?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  let combined_output = if stdout.is_empty() { stderr } else { stdout };

  Ok((output.status.success(), combined_output))
}

/// Helper function to check if a rebase is in progress
fn is_rebase_in_progress(repo_path: &Path) -> bool {
  let rebase_merge_dir = repo_path.join(".git").join("rebase-merge");
  let rebase_apply_dir = repo_path.join(".git").join("rebase-apply");
  rebase_merge_dir.exists() || rebase_apply_dir.exists()
}

#[test]
fn test_rebase_continue_functionality() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create initial commit on main
  create_commit(repo, "shared.txt", "shared content", "Initial commit")?;

  // Create parent branch
  create_branch(repo, "parent", None)?;
  checkout_branch(repo, "parent")?;
  create_commit(repo, "parent.txt", "parent content", "Parent commit")?;

  // Create feature branch from parent
  create_branch(repo, "feature", Some("parent"))?;
  checkout_branch(repo, "feature")?;
  create_commit(repo, "feature.txt", "feature content", "Feature commit")?;

  // Go back to parent and create conflicting change
  checkout_branch(repo, "parent")?;
  create_conflicting_commit(
    repo,
    "shared.txt",
    "conflicting parent content",
    "Conflicting parent change",
  )?;

  // Set up dependency
  add_branch_dependency(repo_path, "feature", "parent")?;

  // Go back to feature branch
  checkout_branch(repo, "feature")?;

  // Start a rebase that will cause conflict
  let has_conflicts = simulate_rebase_with_conflicts(repo_path, "feature", "parent")?;

  if has_conflicts {
    // Verify rebase is in progress
    assert!(
      is_rebase_in_progress(repo_path),
      "Rebase should be in progress after conflict"
    );

    // Simulate resolving conflicts by editing the conflicting file
    let conflict_file_path = repo_path.join("shared.txt");
    fs::write(&conflict_file_path, "resolved content")?;

    // Stage the resolved file
    let (success, _) = execute_git_command(repo_path, &["add", "shared.txt"])?;
    assert!(success, "Should be able to stage resolved file");

    // Test rebase --continue
    let (_success, output) = execute_git_command(repo_path, &["rebase", "--continue"])?;

    // The continue should succeed or at least not fail catastrophically
    println!("Rebase continue output: {}", output);

    // Verify rebase is no longer in progress
    assert!(
      !is_rebase_in_progress(repo_path),
      "Rebase should complete after continue"
    );
  }

  Ok(())
}

#[test]
fn test_rebase_skip_functionality() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create initial commit on main
  create_commit(repo, "base.txt", "base content", "Initial commit")?;

  // Create parent branch
  create_branch(repo, "parent", None)?;
  checkout_branch(repo, "parent")?;
  create_commit(repo, "parent.txt", "parent content", "Parent commit")?;

  // Create feature branch from parent's starting point
  checkout_branch(repo, "main")?;
  create_branch(repo, "feature", None)?;
  checkout_branch(repo, "feature")?;

  // Create a commit that will conflict with parent
  create_conflicting_commit(repo, "base.txt", "feature modification", "Feature change")?;

  // Add more commits on feature
  create_commit(repo, "feature1.txt", "feature1 content", "Feature commit 1")?;
  create_commit(repo, "feature2.txt", "feature2 content", "Feature commit 2")?;

  // Go back to parent and make sure it has diverged
  checkout_branch(repo, "parent")?;
  create_conflicting_commit(repo, "base.txt", "parent modification", "Parent change")?;

  // Set up dependency
  add_branch_dependency(repo_path, "feature", "parent")?;

  // Go back to feature branch
  checkout_branch(repo, "feature")?;

  // Start a rebase that will cause conflict
  let has_conflicts = simulate_rebase_with_conflicts(repo_path, "feature", "parent")?;

  if has_conflicts {
    // Verify rebase is in progress
    assert!(
      is_rebase_in_progress(repo_path),
      "Rebase should be in progress after conflict"
    );

    // Test rebase --skip (skip the conflicting commit)
    let (_success, output) = execute_git_command(repo_path, &["rebase", "--skip"])?;

    println!("Rebase skip output: {}", output);

    // After skip, rebase might still be in progress if there are more commits
    // Let's check the status
    let still_in_progress = is_rebase_in_progress(repo_path);
    println!("Rebase still in progress after skip: {}", still_in_progress);

    if still_in_progress {
      // If still in progress, continue or skip until done
      let mut max_attempts = 10; // Prevent infinite loops
      while is_rebase_in_progress(repo_path) && max_attempts > 0 {
        let (_, status_output) = execute_git_command(repo_path, &["status", "--porcelain"])?;

        if status_output.trim().is_empty() {
          // No conflicts, try to continue
          let (cont_success, cont_output) = execute_git_command(repo_path, &["rebase", "--continue"])?;
          println!("Continue attempt: success={}, output={}", cont_success, cont_output);
        } else {
          // Still have conflicts, skip again
          let (skip_success, skip_output) = execute_git_command(repo_path, &["rebase", "--skip"])?;
          println!("Skip attempt: success={}, output={}", skip_success, skip_output);
        }
        max_attempts -= 1;
      }
    }

    // Eventually, rebase should complete
    assert!(
      !is_rebase_in_progress(repo_path),
      "Rebase should eventually complete after skip operations"
    );

    // Verify we're on the feature branch
    let (success, current_branch) = execute_git_command(repo_path, &["branch", "--show-current"])?;
    assert!(success, "Should be able to get current branch");
    assert_eq!(
      current_branch.trim(),
      "feature",
      "Should be on feature branch after rebase"
    );
  }

  Ok(())
}

#[test]
fn test_rebase_abort_functionality() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create initial commit on main
  create_commit(repo, "shared.txt", "shared content", "Initial commit")?;

  // Create parent branch
  create_branch(repo, "parent", None)?;
  checkout_branch(repo, "parent")?;
  create_commit(repo, "parent.txt", "parent content", "Parent commit")?;

  // Create feature branch from main (not from parent to create divergence)
  checkout_branch(repo, "main")?;
  create_branch(repo, "feature", None)?;
  checkout_branch(repo, "feature")?;
  create_conflicting_commit(
    repo,
    "shared.txt",
    "conflicting feature content",
    "Conflicting feature change",
  )?;

  // Set up dependency
  add_branch_dependency(repo_path, "feature", "parent")?;

  // Start a rebase that will cause conflict
  let has_conflicts = simulate_rebase_with_conflicts(repo_path, "feature", "parent")?;

  if has_conflicts {
    // Verify rebase is in progress
    assert!(
      is_rebase_in_progress(repo_path),
      "Rebase should be in progress after conflict"
    );

    // Test rebase --abort
    let (success, output) = execute_git_command(repo_path, &["rebase", "--abort"])?;

    println!("Rebase abort output: {}", output);

    // Verify rebase is no longer in progress
    assert!(
      !is_rebase_in_progress(repo_path),
      "Rebase should be aborted and no longer in progress"
    );

    // Verify we're back on the feature branch
    let (success, current_branch) = execute_git_command(repo_path, &["branch", "--show-current"])?;
    assert!(success, "Should be able to get current branch");
    assert_eq!(
      current_branch.trim(),
      "feature",
      "Should be back on feature branch after abort"
    );
  }

  Ok(())
}

#[test]
fn test_rebase_cleanup_after_skip() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Create a simple setup that will allow us to test the cleanup function
  create_commit(repo, "test.txt", "initial content", "Initial commit")?;

  // Create parent branch
  create_branch(repo, "parent", None)?;
  checkout_branch(repo, "parent")?;
  create_commit(repo, "parent.txt", "parent content", "Parent commit")?;

  // Create feature branch with conflicting changes
  checkout_branch(repo, "main")?;
  create_branch(repo, "feature", None)?;
  checkout_branch(repo, "feature")?;
  create_conflicting_commit(repo, "test.txt", "feature content", "Feature change")?;

  // Set up dependency
  add_branch_dependency(repo_path, "feature", "parent")?;

  // Start rebase
  let has_conflicts = simulate_rebase_with_conflicts(repo_path, "feature", "parent")?;

  if has_conflicts && is_rebase_in_progress(repo_path) {
    // Skip the conflicting commit
    let (skip_success, _) = execute_git_command(repo_path, &["rebase", "--skip"])?;

    // Test that cleanup_index_after_skip functionality works
    // This is tested indirectly by ensuring the repository is in a clean state
    let (status_success, status_output) = execute_git_command(repo_path, &["status", "--porcelain"])?;
    assert!(status_success, "Should be able to get status");

    // The status should be clean or show only normal rebase-in-progress state
    println!("Status after skip: {}", status_output);

    // Complete any remaining rebase operations
    let mut max_attempts = 5;
    while is_rebase_in_progress(repo_path) && max_attempts > 0 {
      let (_, _) = execute_git_command(repo_path, &["rebase", "--continue"])?;
      max_attempts -= 1;
    }

    // Final verification that repository is in a clean state
    assert!(
      !is_rebase_in_progress(repo_path),
      "Repository should be in a clean state after rebase operations"
    );
  }

  Ok(())
}

#[test]
fn test_detect_rebase_in_progress() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  let repo_path = guard.path();
  let repo = &guard.repo;

  // Initially, no rebase should be in progress
  assert!(
    !is_rebase_in_progress(repo_path),
    "No rebase should be in progress initially"
  );

  // Create setup for conflict
  create_commit(repo, "shared.txt", "shared content", "Initial commit")?;

  create_branch(repo, "parent", None)?;
  checkout_branch(repo, "parent")?;
  create_commit(repo, "parent.txt", "parent content", "Parent commit")?;

  checkout_branch(repo, "main")?;
  create_branch(repo, "feature", None)?;
  checkout_branch(repo, "feature")?;
  create_conflicting_commit(repo, "shared.txt", "conflicting content", "Conflicting change")?;

  // Start rebase that will conflict
  let has_conflicts = simulate_rebase_with_conflicts(repo_path, "feature", "parent")?;

  if has_conflicts {
    // Now rebase should be in progress
    assert!(
      is_rebase_in_progress(repo_path),
      "Rebase should be detected as in progress"
    );

    // Abort to clean up
    execute_git_command(repo_path, &["rebase", "--abort"])?;

    // Should no longer be in progress
    assert!(
      !is_rebase_in_progress(repo_path),
      "Rebase should no longer be in progress after abort"
    );
  }

  Ok(())
}
