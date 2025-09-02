//! # Update Command
//!
//! Implementation of the update command that switches to the root branch,
//! fetches from origin, pulls the latest commits, and runs the twig cascade command.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::{RepoState, detect_repository};

use super::cascade::{self, CascadeArgs};

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

/// Handle the update command
pub fn handle_update_command(args: UpdateArgs) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(ref repo_arg) = args.repo {
    PathBuf::from(repo_arg)
  } else {
    detect_repository().context("Not in a git repository")?
  };

  print_info("🔄 Starting repository update...");

  // Check for uncommitted changes and stash if autostash is enabled
  let mut stash_created = false;
  if args.autostash {
    if has_uncommitted_changes(&repo_path)? {
      print_info("💾 Stashing uncommitted changes...");
      let stash_result = Command::new("git")
        .current_dir(&repo_path)
        .args(&["stash", "push", "-m", "twig update autostash"])
        .output()
        .context("Failed to execute git stash")?;

      if !stash_result.status.success() {
        let error_output = String::from_utf8_lossy(&stash_result.stderr);
        print_error(&format!("Failed to stash changes: {}", error_output));
        return Err(anyhow::anyhow!("Git stash failed"));
      } else {
        print_success("✓ Changes stashed");
        stash_created = true;
      }
    }
  }

  // Load repository state to find root branches
  let repo_state = RepoState::load(&repo_path).unwrap_or_default();
  let root_branches = repo_state.get_root_branches();

  // Determine which branch to switch to
  let target_branch = if !root_branches.is_empty() {
    // Use the first root branch (or default if one exists)
    let default_root = repo_state.get_default_root();
    let candidate = if let Some(default) = default_root {
      default.to_string()
    } else {
      root_branches[0].clone()
    };
    
    // Verify the root branch actually exists
    if branch_exists(&repo_path, &candidate)? {
      candidate
    } else {
      print_warning(&format!("Configured root branch '{}' does not exist, falling back to default", candidate));
      // Fall back to common default branches
      if branch_exists(&repo_path, "main")? {
        "main".to_string()
      } else if branch_exists(&repo_path, "master")? {
        "master".to_string()
      } else {
        return Err(anyhow::anyhow!(
          "Configured root branch '{}' does not exist and no 'main' or 'master' branch found.\n\
           Please ensure your root branch exists or configure a valid root branch using: twig branch root add <branch-name>",
          candidate
        ));
      }
    }
  } else {
    // Fall back to common default branches
    if branch_exists(&repo_path, "main")? {
      "main".to_string()
    } else if branch_exists(&repo_path, "master")? {
      "master".to_string()
    } else {
      return Err(anyhow::anyhow!(
        "No root branches configured and no 'main' or 'master' branch found.\n\
         Please configure a root branch using: twig branch root add <branch-name>"
      ));
    }
  };

  print_info(&format!("📍 Switching to root branch: {}", target_branch));

  // Switch to the root branch
  let switch_result = Command::new("git")
    .current_dir(&repo_path)
    .args(&["checkout", &target_branch])
    .output()
    .context("Failed to execute git checkout")?;

  if !switch_result.status.success() {
    let error_output = String::from_utf8_lossy(&switch_result.stderr);
    print_error(&format!("Failed to switch to branch '{}': {}", target_branch, error_output));
    return Err(anyhow::anyhow!("Git checkout failed"));
  }

  print_success(&format!("✓ Switched to branch '{}'", target_branch));

  // Fetch from origin
  print_info("🌐 Fetching from origin...");
  let fetch_result = Command::new("git")
    .current_dir(&repo_path)
    .args(&["fetch", "origin"])
    .output()
    .context("Failed to execute git fetch")?;

  if !fetch_result.status.success() {
    let error_output = String::from_utf8_lossy(&fetch_result.stderr);
    print_warning(&format!("Fetch completed with warnings: {}", error_output));
  } else {
    print_success("✓ Fetched latest changes from origin");
  }

  // Pull latest commits
  print_info("⬇️  Pulling latest commits...");
  let pull_result = Command::new("git")
    .current_dir(&repo_path)
    .args(&["pull", "origin", &target_branch])
    .output()
    .context("Failed to execute git pull")?;

  if !pull_result.status.success() {
    let error_output = String::from_utf8_lossy(&pull_result.stderr);
    if error_output.contains("Already up to date") || error_output.contains("Already up-to-date") {
      print_info("✓ Branch is already up to date");
    } else {
      print_error(&format!("Failed to pull latest commits: {}", error_output));
      return Err(anyhow::anyhow!("Git pull failed"));
    }
  } else {
    let pull_output = String::from_utf8_lossy(&pull_result.stdout);
    if pull_output.contains("Already up to date") || pull_output.contains("Already up-to-date") {
      print_info("✓ Branch is already up to date");
    } else {
      print_success("✓ Pulled latest commits");
    }
  }

  // Run cascade command if not disabled
  if !args.no_cascade {
    print_info("🌊 Running cascade to update dependent branches...");
    
    let cascade_args = CascadeArgs {
      max_depth: None,
      force: args.force_cascade,
      show_graph: args.show_graph,
      autostash: args.autostash,
      repo: args.repo,
    };

    match cascade::handle_cascade_command(cascade_args) {
      Ok(()) => print_success("✓ Cascade completed successfully"),
      Err(e) => {
        print_warning(&format!("Cascade completed with issues: {}", e));
        // Don't fail the entire update command if cascade has issues
      }
    }
  }

  // Pop stash if we created one
  if stash_created {
    print_info("📤 Restoring stashed changes...");
    let pop_result = Command::new("git")
      .current_dir(&repo_path)
      .args(&["stash", "pop"])
      .output()
      .context("Failed to execute git stash pop")?;

    if !pop_result.status.success() {
      let error_output = String::from_utf8_lossy(&pop_result.stderr);
      print_warning(&format!("Failed to restore stashed changes: {}", error_output));
      print_warning("You may need to manually restore changes with 'git stash pop'");
    } else {
      print_success("✓ Stashed changes restored");
    }
  }

  print_success("🎉 Repository update completed!");
  Ok(())
}

/// Check if a branch exists in the repository
fn branch_exists(repo_path: &PathBuf, branch_name: &str) -> Result<bool> {
  let result = Command::new("git")
    .current_dir(repo_path)
    .args(&["show-ref", "--verify", "--quiet", &format!("refs/heads/{}", branch_name)])
    .output()
    .context("Failed to check if branch exists")?;

  Ok(result.status.success())
}

/// Check if there are uncommitted changes in the repository
fn has_uncommitted_changes(repo_path: &PathBuf) -> Result<bool> {
  let result = Command::new("git")
    .current_dir(repo_path)
    .args(&["diff-index", "--quiet", "HEAD", "--"])
    .output()
    .context("Failed to check for uncommitted changes")?;

  // git diff-index returns 0 if no changes, 1 if there are changes
  Ok(!result.status.success())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::TempDir;
  use git2::Repository;

  fn create_test_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();
    
    // Initialize git repository
    Repository::init(&repo_path).unwrap();
    
    // Configure git user (required for commits)
    Command::new("git")
      .current_dir(&repo_path)
      .args(&["config", "user.name", "Test User"])
      .output()
      .unwrap();
    
    Command::new("git")
      .current_dir(&repo_path)
      .args(&["config", "user.email", "test@example.com"])
      .output()
      .unwrap();

    // Create initial commit
    fs::write(repo_path.join("README.md"), "# Test Repository").unwrap();
    Command::new("git")
      .current_dir(&repo_path)
      .args(&["add", "README.md"])
      .output()
      .unwrap();
    Command::new("git")
      .current_dir(&repo_path)
      .args(&["commit", "-m", "Initial commit"])
      .output()
      .unwrap();

    (temp_dir, repo_path)
  }

  #[test]
  fn test_branch_exists() {
    let (_temp_dir, repo_path) = create_test_repo();

    // main branch should exist (created by default in modern git)
    // But let's check for master first as it might be the default
    let has_main = branch_exists(&repo_path, "main").unwrap();
    let has_master = branch_exists(&repo_path, "master").unwrap();
    
    // At least one should exist
    assert!(has_main || has_master);

    // Non-existent branch should return false
    assert!(!branch_exists(&repo_path, "non-existent-branch").unwrap());
  }
}
