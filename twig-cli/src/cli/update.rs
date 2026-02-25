//! # Update Command
//!
//! Implementation of the update command that switches to the root branch,
//! fetches from origin, pulls the latest commits, and runs the twig cascade
//! command.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::{RepoState, git::current_branch};

use super::cascade::{self, CascadeArgs};
use crate::consts;

/// Command for updating the repository and cascading changes
#[derive(Args)]
pub struct UpdateArgs {
  /// Path to a specific repository
  #[arg(short, long, value_name = "PATH")]
  pub repo: Option<String>,

  /// Skip the cascade operation after updating
  #[arg(long)]
  pub no_cascade: bool,

  /// Force cascade even if branches are up-to-date
  #[arg(long)]
  pub force_cascade: bool,

  /// Show dependency graph before cascading
  #[arg(long = "show-graph")]
  pub show_graph: bool,

  /// Automatically stash and pop pending changes
  #[arg(long)]
  pub autostash: bool,
}

/// Handle the update command.
///
/// Resolves the target repository, optionally stashes dirty working-tree
/// changes, switches to the configured root branch, fetches and pulls from
/// origin, then optionally runs a cascading rebase over all dependent
/// branches.  When `--autostash` is set the stash is always popped on the
/// branch that was active when the command was invoked.
///
/// # Errors
///
/// Returns an error if the repository path cannot be resolved, if any
/// required git operation (checkout, fetch, pull) fails, or if the stash
/// cannot be applied after a failed branch restore.
///
/// # Side Effects
///
/// - May switch the current branch to the root branch.
/// - May create and pop a git stash entry.
/// - May rebase one or more descendant branches (unless `--no-cascade`).
pub fn handle_update_command(args: UpdateArgs) -> Result<()> {
  let repo_path = crate::utils::resolve_repository_path(args.repo.as_deref())?;

  print_info("Starting repository update...");

  // Record the branch we started on so we can return to it before stash pop.
  let original_branch = current_branch().ok().flatten();

  // Check for uncommitted changes and stash if autostash is enabled
  let mut stash_created = false;
  if args.autostash && has_uncommitted_changes(&repo_path)? {
    print_info("Stashing uncommitted changes...");
    tracing::debug!("Running git stash push in {:?}", repo_path);
    let stash_result = Command::new(consts::GIT_EXECUTABLE)
      .current_dir(&repo_path)
      .args(["stash", "push", "-m", "twig update autostash"])
      .output()
      .context("Failed to execute git stash")?;

    if !stash_result.status.success() {
      let error_output = String::from_utf8_lossy(&stash_result.stderr);
      print_error(&format!("Failed to stash changes: {}", error_output));
      return Err(anyhow::anyhow!("Git stash failed"));
    }
    print_success("Changes stashed");
    stash_created = true;
  }

  // Load repository state to find root branches; warn and fall back to
  // defaults if the state file is missing or malformed.
  let repo_state = RepoState::load(&repo_path)
    .inspect_err(|e| tracing::warn!("Failed to load repo state, using defaults: {}", e))
    .unwrap_or_default();
  let root_branches = repo_state.get_root_branches();

  // Determine which branch to switch to
  let target_branch = determine_target_branch(&repo_path, &repo_state, &root_branches)?;

  print_info(&format!("Switching to root branch: {}", target_branch));

  // Switch to the root branch
  tracing::debug!("Running git checkout {} in {:?}", target_branch, repo_path);
  let switch_result = Command::new(consts::GIT_EXECUTABLE)
    .current_dir(&repo_path)
    .args(["checkout", &target_branch])
    .output()
    .context("Failed to execute git checkout")?;

  if !switch_result.status.success() {
    let error_output = String::from_utf8_lossy(&switch_result.stderr);
    print_error(&format!(
      "Failed to switch to branch '{}': {}",
      target_branch, error_output
    ));
    return Err(anyhow::anyhow!("Git checkout failed"));
  }

  print_success(&format!("Switched to branch '{}'", target_branch));

  // Fetch from origin
  print_info("Fetching from origin...");
  tracing::debug!("Running git fetch origin in {:?}", repo_path);
  let fetch_result = Command::new(consts::GIT_EXECUTABLE)
    .current_dir(&repo_path)
    .args(["fetch", "origin"])
    .output()
    .context("Failed to execute git fetch")?;

  if !fetch_result.status.success() {
    let error_output = String::from_utf8_lossy(&fetch_result.stderr);
    print_warning(&format!("Fetch completed with warnings: {}", error_output));
  } else {
    let fetch_output = String::from_utf8_lossy(&fetch_result.stdout);
    tracing::debug!("git fetch output: {}", fetch_output);
    print_success("Fetched latest changes from origin");
  }

  // Pull latest commits
  print_info("Pulling latest commits...");
  tracing::debug!("Running git pull origin {} in {:?}", target_branch, repo_path);
  let pull_result = Command::new(consts::GIT_EXECUTABLE)
    .current_dir(&repo_path)
    .args(["pull", "origin", &target_branch])
    .output()
    .context("Failed to execute git pull")?;

  if !pull_result.status.success() {
    let error_output = String::from_utf8_lossy(&pull_result.stderr);
    if error_output.contains("Already up to date") || error_output.contains("Already up-to-date") {
      print_info("Branch is already up to date");
    } else {
      print_error(&format!("Failed to pull latest commits: {}", error_output));
      return Err(anyhow::anyhow!("Git pull failed"));
    }
  } else {
    let pull_output = String::from_utf8_lossy(&pull_result.stdout);
    tracing::debug!("git pull output: {}", pull_output);
    if pull_output.contains("Already up to date") || pull_output.contains("Already up-to-date") {
      print_info("Branch is already up to date");
    } else {
      print_success("Pulled latest commits");
    }
  }

  // Run cascade command if not disabled
  if !args.no_cascade {
    print_info("Running cascade to update dependent branches...");

    let cascade_args = CascadeArgs {
      max_depth: None,
      force: args.force_cascade,
      show_graph: args.show_graph,
      autostash: args.autostash,
      preview: false,
      repo: args.repo.clone(),
    };

    match cascade::handle_cascade_command(cascade_args) {
      Ok(()) => print_success("Cascade completed successfully"),
      Err(e) => {
        print_warning(&format!("Cascade completed with issues: {}", e));
        // Don't fail the entire update command if cascade has issues
      }
    }
  }

  // Pop stash if we created one — restore original branch first so the
  // stash lands on the right branch.
  if stash_created {
    if let Some(ref branch) = original_branch {
      tracing::debug!("Switching back to original branch '{}' before stash pop", branch);
      let restore_result = Command::new(consts::GIT_EXECUTABLE)
        .current_dir(&repo_path)
        .args(["checkout", branch])
        .output()
        .context("Failed to switch back to original branch")?;

      if !restore_result.status.success() {
        let error_output = String::from_utf8_lossy(&restore_result.stderr);
        print_warning(&format!(
          "Could not switch back to original branch '{}': {}",
          branch, error_output
        ));
        print_warning("Your stash is still saved — run 'git stash pop' manually on the correct branch");
        return Ok(());
      }
    }

    print_info("Restoring stashed changes...");
    tracing::debug!("Running git stash pop in {:?}", repo_path);
    let pop_result = Command::new(consts::GIT_EXECUTABLE)
      .current_dir(&repo_path)
      .args(["stash", "pop"])
      .output()
      .context("Failed to execute git stash pop")?;

    if !pop_result.status.success() {
      let error_output = String::from_utf8_lossy(&pop_result.stderr);
      print_warning(&format!(
        "Failed to restore stashed changes: {}",
        error_output
      ));
      print_warning("You may need to manually restore changes with 'git stash pop'");
    } else {
      print_success("Stashed changes restored");
    }
  }

  print_success("Repository update completed!");
  Ok(())
}

/// Determine the target root branch to switch to for the update.
///
/// Checks configured root branches first, then falls back to `main` or
/// `master`.
fn determine_target_branch(
  repo_path: &Path,
  repo_state: &RepoState,
  root_branches: &[String],
) -> Result<String> {
  if !root_branches.is_empty() {
    let default_root = repo_state.get_default_root();
    let candidate = if let Some(default) = default_root {
      default.to_string()
    } else {
      root_branches[0].clone()
    };

    if branch_exists(repo_path, &candidate)? {
      return Ok(candidate);
    }

    print_warning(&format!(
      "Configured root branch '{}' does not exist, falling back to default",
      candidate
    ));
  }

  // Fall back to common default branches
  if branch_exists(repo_path, "main")? {
    Ok("main".to_string())
  } else if branch_exists(repo_path, "master")? {
    Ok("master".to_string())
  } else {
    Err(anyhow::anyhow!(
      "No root branches configured and no 'main' or 'master' branch found.\n\
       Please configure a root branch using: twig branch root add <branch-name>"
    ))
  }
}

/// Check if a local branch exists in the repository at `repo_path`.
///
/// Uses `git2` directly rather than shelling out, matching the approach used
/// by `twig_core::git::branch_exists`.
fn branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool> {
  let repo = git2::Repository::open(repo_path).context("Failed to open git repository")?;
  match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(_) => Ok(true),
    Err(_) => Ok(false),
  }
}

/// Check if there are uncommitted changes in the repository
fn has_uncommitted_changes(repo_path: &Path) -> Result<bool> {
  tracing::debug!("Checking for uncommitted changes in {:?}", repo_path);
  let result = Command::new(consts::GIT_EXECUTABLE)
    .current_dir(repo_path)
    .args(["diff-index", "--quiet", "HEAD", "--"])
    .output()
    .context("Failed to check for uncommitted changes")?;

  // git diff-index returns 0 if no changes, 1 if there are changes
  Ok(!result.status.success())
}

#[cfg(test)]
mod tests {
  use std::fs;

  use twig_test_utils::{GitRepoTestGuard, create_commit};

  use super::*;

  #[test]
  fn test_branch_exists() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    let repo_path = guard.path().to_path_buf();

    // Create an initial commit so that the branch ref is established
    create_commit(&guard.repo, "README.md", "# Test", "Initial commit")
      .expect("create initial commit");

    // GitRepoTestGuard initialises with "main"
    assert!(branch_exists(&repo_path, "main").expect("check main"));

    // Non-existent branch should return false
    assert!(!branch_exists(&repo_path, "non-existent-branch").expect("check nonexistent"));
  }

  #[test]
  fn test_has_uncommitted_changes_clean() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    let repo_path = guard.path().to_path_buf();

    create_commit(&guard.repo, "README.md", "# Test", "Initial commit")
      .expect("create initial commit");

    // Clean repo should have no uncommitted changes
    let has_changes = has_uncommitted_changes(&repo_path).expect("check changes");
    assert!(!has_changes);
  }

  #[test]
  fn test_has_uncommitted_changes_dirty() {
    let guard = GitRepoTestGuard::new_and_change_dir();
    let repo_path = guard.path().to_path_buf();

    create_commit(&guard.repo, "README.md", "# Test", "Initial commit")
      .expect("create initial commit");

    // Modify a tracked file to create uncommitted changes
    fs::write(repo_path.join("README.md"), "# Modified").expect("write modified");

    let has_changes = has_uncommitted_changes(&repo_path).expect("check changes");
    assert!(has_changes);
  }
}
