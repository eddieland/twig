#![cfg(unix)]

use std::fs;
use std::path::Path;

use anyhow::Result;
use git2::{BranchType, Repository as Git2Repository, Signature};
use twig_core::state::RepoState;
use twig_test_utils::git::{GitRepoTestGuard, ensure_main_branch};

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

/// Helper function to add a root branch
fn add_root_branch(repo_path: &Path, branch: &str, is_default: bool) -> Result<()> {
  let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
  repo_state.add_root(branch.to_string(), is_default)?;
  repo_state.save(repo_path)?;
  Ok(())
}

/// Helper function to simulate running the rebase command
fn run_rebase_command(repo_path: &Path, force: bool, show_graph: bool, autostash: bool) -> Result<String> {
  use twig_cli::cli::rebase::{RebaseArgs, handle_rebase_command};

  // We don't need to capture output for the test

  // Create the args
  let args = RebaseArgs {
    force,
    show_graph,
    autostash,
    repo: Some(repo_path.to_string_lossy().to_string()),
  };

  // Run the command
  let result = handle_rebase_command(args);

  // Return a string representation of the result
  match result {
    Ok(_) => Ok("Successfully rebased".to_string()),
    Err(e) => Ok(format!("Error: {}", e)),
  }
}

/// Helper function to simulate running the cascade command
fn run_cascade_command(
  repo_path: &Path,
  max_depth: Option<u32>,
  force: bool,
  show_graph: bool,
  autostash: bool,
  preview: bool,
) -> Result<String> {
  use twig_cli::cli::cascade::{CascadeArgs, handle_cascade_command};

  // We don't need to capture output for the test

  // Create the args
  let args = CascadeArgs {
    max_depth,
    force,
    show_graph,
    autostash,
    force_push: false,
    preview,
    repo: Some(repo_path.to_string_lossy().to_string()),
  };

  // Run the command
  let result = handle_cascade_command(args);

  // Return a string representation of the result
  match result {
    Ok(_) => Ok("Cascading rebase completed successfully".to_string()),
    Err(e) => Ok(format!("Error: {}", e)),
  }
}

// This function is not used, so we can remove it

#[test]
fn test_rebase_command() -> Result<()> {
  // Create a temporary git repository
  let git_repo = GitRepoTestGuard::new();
  let repo = &git_repo.repo;
  let repo_path = git_repo.path();

  // Create initial commit on main branch
  create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
  ensure_main_branch(repo)?;

  // Create feature branch
  create_branch(repo, "feature", Some("main"))?;

  // Create another commit on main
  checkout_branch(repo, "main")?;
  create_commit(repo, "file2.txt", "Main branch content", "Main branch commit")?;

  // Create a commit on feature branch
  checkout_branch(repo, "feature")?;
  create_commit(repo, "file3.txt", "Feature branch content", "Feature branch commit")?;

  // Set up branch dependencies
  add_branch_dependency(repo_path, "feature", "main")?;
  add_root_branch(repo_path, "main", true)?;

  // Run the rebase command
  let output = run_rebase_command(repo_path, false, false, false)?;

  // Verify that the rebase was successful
  assert!(output.contains("Successfully rebased") || output.contains("up-to-date"));

  // Verify that feature branch is now based on main
  checkout_branch(repo, "feature")?;
  let feature_commit = repo.head()?.peel_to_commit()?;
  let feature_tree = feature_commit.tree()?;

  // Check that file2.txt from main is now in feature branch
  let entry = feature_tree.get_name("file2.txt");
  assert!(
    entry.is_some(),
    "file2.txt should be present in feature branch after rebase"
  );

  Ok(())
}

#[test]
fn test_cascade_command() -> Result<()> {
  // Create a temporary git repository
  let git_repo = GitRepoTestGuard::new();
  let repo = &git_repo.repo;
  let repo_path = git_repo.path();

  // Create initial commit on main branch
  create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

  ensure_main_branch(repo)?;

  // Create feature branch
  create_branch(repo, "feature", Some("main"))?;
  checkout_branch(repo, "feature")?;
  create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

  // Create sub-feature branch from feature
  create_branch(repo, "sub-feature", Some("feature"))?;
  checkout_branch(repo, "sub-feature")?;
  create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

  // Create another sub-feature branch
  create_branch(repo, "sub-feature-2", Some("feature"))?;
  checkout_branch(repo, "sub-feature-2")?;
  create_commit(
    repo,
    "sub-feature-2.txt",
    "Sub-feature 2 content",
    "Sub-feature 2 commit",
  )?;

  // Go back to main and create another commit
  checkout_branch(repo, "main")?;
  create_commit(repo, "main-update.txt", "Updated main content", "Updated main commit")?;

  // Set up branch dependencies
  add_branch_dependency(repo_path, "feature", "main")?;
  add_branch_dependency(repo_path, "sub-feature", "feature")?;
  add_branch_dependency(repo_path, "sub-feature-2", "feature")?;
  add_root_branch(repo_path, "main", true)?;

  // Checkout feature branch and run cascade command
  checkout_branch(repo, "feature")?;

  // First rebase feature onto main
  run_rebase_command(repo_path, false, false, false)?;

  // Then cascade from feature to its children
  let output = run_cascade_command(repo_path, None, false, false, false, false)?;

  // Verify that the cascade was successful
  assert!(
    output.contains("Cascading rebase completed successfully")
      || output.contains("Successfully rebased")
      || output.contains("up-to-date")
  );

  // Verify that sub-feature branches have the changes from main
  checkout_branch(repo, "sub-feature")?;
  let sub_feature_tree = repo.head()?.peel_to_commit()?.tree()?;
  let entry = sub_feature_tree.get_name("main-update.txt");
  assert!(
    entry.is_some(),
    "main-update.txt should be present in sub-feature branch after cascade"
  );

  checkout_branch(repo, "sub-feature-2")?;
  let sub_feature_2_tree = repo.head()?.peel_to_commit()?.tree()?;
  let entry = sub_feature_2_tree.get_name("main-update.txt");
  assert!(
    entry.is_some(),
    "main-update.txt should be present in sub-feature-2 branch after cascade"
  );

  Ok(())
}

#[test]
fn test_rebase_with_force_flag() -> Result<()> {
  // Create a temporary git repository
  let git_repo = GitRepoTestGuard::new();
  let repo = &git_repo.repo;
  let repo_path = git_repo.path();

  // Create initial commit on main branch
  create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

  ensure_main_branch(repo)?;

  // Create feature branch
  create_branch(repo, "feature", Some("main"))?;

  // Set up branch dependencies
  add_branch_dependency(repo_path, "feature", "main")?;
  add_root_branch(repo_path, "main", true)?;

  // Checkout feature branch
  checkout_branch(repo, "feature")?;

  // Run the rebase command with force flag
  let output = run_rebase_command(repo_path, true, false, false)?;

  // Verify that the rebase was attempted even though branches are up-to-date
  assert!(output.contains("force flag is set") || output.contains("Successfully rebased"));

  Ok(())
}

#[test]
fn test_cascade_with_max_depth() -> Result<()> {
  // Create a temporary git repository
  let git_repo = GitRepoTestGuard::new();
  let repo = &git_repo.repo;
  let repo_path = git_repo.path();

  // Create initial commit on main branch
  create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

  ensure_main_branch(repo)?;

  // Create feature branch
  create_branch(repo, "feature", Some("main"))?;
  checkout_branch(repo, "feature")?;
  create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

  // Create sub-feature branch from feature
  create_branch(repo, "sub-feature", Some("feature"))?;
  checkout_branch(repo, "sub-feature")?;
  create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

  // Create sub-sub-feature branch from sub-feature
  create_branch(repo, "sub-sub-feature", Some("sub-feature"))?;
  checkout_branch(repo, "sub-sub-feature")?;
  create_commit(
    repo,
    "sub-sub-feature.txt",
    "Sub-sub-feature content",
    "Sub-sub-feature commit",
  )?;

  // Go back to main and create another commit
  checkout_branch(repo, "main")?;
  create_commit(repo, "main-update.txt", "Updated main content", "Updated main commit")?;

  // Set up branch dependencies
  add_branch_dependency(repo_path, "feature", "main")?;
  add_branch_dependency(repo_path, "sub-feature", "feature")?;
  add_branch_dependency(repo_path, "sub-sub-feature", "sub-feature")?;
  add_root_branch(repo_path, "main", true)?;

  // Checkout feature branch and run cascade command with max-depth=1
  checkout_branch(repo, "feature")?;

  // First rebase feature onto main
  run_rebase_command(repo_path, false, false, false)?;

  // Then cascade from feature to its children with max-depth=1
  let output = run_cascade_command(repo_path, Some(1), false, false, false, false)?;

  // Verify that the cascade was successful
  assert!(
    output.contains("Cascading rebase completed successfully")
      || output.contains("Successfully rebased")
      || output.contains("up-to-date")
  );

  // Verify that sub-feature has the changes from main
  checkout_branch(repo, "sub-feature")?;
  let sub_feature_tree = repo.head()?.peel_to_commit()?.tree()?;
  let entry = sub_feature_tree.get_name("main-update.txt");
  assert!(
    entry.is_some(),
    "main-update.txt should be present in sub-feature branch after cascade"
  );

  // Verify that sub-sub-feature does NOT have the changes from main (due to
  // max-depth=1)
  checkout_branch(repo, "sub-sub-feature")?;
  let sub_sub_feature_tree = repo.head()?.peel_to_commit()?.tree()?;
  let entry = sub_sub_feature_tree.get_name("main-update.txt");
  assert!(
    entry.is_none(),
    "main-update.txt should NOT be present in sub-sub-feature branch due to max-depth=1"
  );

  Ok(())
}

#[test]
fn test_cascade_preview() -> Result<()> {
  // Create a temporary git repository
  let git_repo = GitRepoTestGuard::new();
  let repo = &git_repo.repo;
  let repo_path = git_repo.path();

  // Create initial commit on main branch
  create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

  ensure_main_branch(repo)?;

  // Create feature branch
  create_branch(repo, "feature", Some("main"))?;
  checkout_branch(repo, "feature")?;
  create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

  // Create sub-feature branch from feature
  create_branch(repo, "sub-feature", Some("feature"))?;
  checkout_branch(repo, "sub-feature")?;
  create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

  // Go back to main and create another commit
  checkout_branch(repo, "main")?;
  create_commit(repo, "main-update.txt", "Updated main content", "Updated main commit")?;

  // Set up branch dependencies
  add_branch_dependency(repo_path, "feature", "main")?;
  add_branch_dependency(repo_path, "sub-feature", "feature")?;
  add_root_branch(repo_path, "main", true)?;

  // Checkout feature branch
  checkout_branch(repo, "feature")?;

  // Record the commit SHAs before preview
  let feature_sha = repo.head()?.peel_to_commit()?.id();
  checkout_branch(repo, "sub-feature")?;
  let sub_feature_sha = repo.head()?.peel_to_commit()?.id();
  checkout_branch(repo, "feature")?;

  // Run cascade with --preview
  let output = run_cascade_command(repo_path, None, false, false, false, true)?;

  // Preview should succeed without error
  assert!(
    output.contains("Cascading rebase completed successfully"),
    "Preview should return Ok: got {output}"
  );

  // Verify that no branches were actually modified
  checkout_branch(repo, "feature")?;
  let feature_sha_after = repo.head()?.peel_to_commit()?.id();
  assert_eq!(
    feature_sha, feature_sha_after,
    "feature branch should not be modified by preview"
  );

  checkout_branch(repo, "sub-feature")?;
  let sub_feature_sha_after = repo.head()?.peel_to_commit()?.id();
  assert_eq!(
    sub_feature_sha, sub_feature_sha_after,
    "sub-feature branch should not be modified by preview"
  );

  // Verify that sub-feature does NOT have changes from main (no rebase happened)
  let sub_feature_tree = repo.head()?.peel_to_commit()?.tree()?;
  let entry = sub_feature_tree.get_name("main-update.txt");
  assert!(
    entry.is_none(),
    "main-update.txt should NOT be present in sub-feature after preview (no rebase should occur)"
  );

  Ok(())
}

/// When `--force-push` is requested but the repo has no configured remote,
/// the cascade must report an error rather than silently succeeding.
#[test]
fn test_cascade_force_push_fails_without_remote() -> Result<()> {
  use twig_cli::cli::cascade::{CascadeArgs, handle_cascade_command};

  let git_repo = GitRepoTestGuard::new();
  let repo = &git_repo.repo;
  let repo_path = git_repo.path();

  // Create initial commit on main and a child branch so there is something to cascade.
  create_commit(repo, "base.txt", "base", "Initial commit")?;
  ensure_main_branch(repo)?;
  create_branch(repo, "feature", Some("main"))?;
  checkout_branch(repo, "main")?;
  create_commit(repo, "main2.txt", "update", "Main update")?;
  checkout_branch(repo, "feature")?;
  create_commit(repo, "feat.txt", "feature", "Feature commit")?;
  checkout_branch(repo, "main")?;

  add_root_branch(repo_path, "main", true)?;
  add_branch_dependency(repo_path, "feature", "main")?;

  let args = CascadeArgs {
    max_depth: None,
    force: false,
    show_graph: false,
    autostash: false,
    force_push: true,
    preview: false,
    repo: Some(repo_path.to_string_lossy().to_string()),
  };

  // The cascade can succeed in rebasing but must fail when it tries to push
  // (no remote is configured in the temp repo).
  let result = handle_cascade_command(args);
  assert!(
    result.is_err(),
    "Expected cascade to fail when --force-push is set but no remote is configured"
  );

  Ok(())
}
