//! # Worktree Command
//!
//! Derive-based implementation of the worktree command for managing Git
//! worktrees for efficient multi-branch development.

use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use git2::Repository as Git2Repository;
use twig_core::output::{format_command, format_timestamp, print_header};
use twig_core::state::create_worktree;
use twig_core::{RepoState, detect_repository_from_path, format_repo_path, print_success, print_warning};

use crate::complete::all_branch_completer;

/// Command for worktree management
#[derive(Args)]
pub struct WorktreeArgs {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: WorktreeSubcommands,
}

/// Subcommands for the worktree command
#[derive(Subcommand)]
pub enum WorktreeSubcommands {
  /// Clean up stale worktrees
  #[command(
    long_about = "Removes worktrees that are no longer needed or have been abandoned.\n\n\
                     This helps keep your workspace clean and organized. The command checks for\n\
                     worktrees with branches that have been merged or deleted and offers to\n\
                     remove them. This operation only removes the worktree directories and\n\
                     doesn't affect the main repository."
  )]
  Clean(CleanCommand),

  /// Create a new worktree for a branch
  #[command(long_about = "Creates a new Git worktree for a specific branch.\n\n\
                     This allows you to work on multiple branches simultaneously without switching\n\
                     branches in your main repository. If the branch doesn't exist, it will be\n\
                     created. The worktree will be created in a directory named after the branch\n\
                     within the repository's parent directory.")]
  #[command(alias = "new")]
  Create(CreateCommand),

  /// List all worktrees for a repository
  #[command(long_about = "Lists all worktrees associated with a Git repository.\n\n\
                     Shows the path, branch name, and commit information for each worktree\n\
                     to help you track your active development environments.")]
  #[command(alias = "ls")]
  List(ListCommand),
}

/// Create a new worktree for a branch
#[derive(Args)]
pub struct CreateCommand {
  /// Branch name
  #[arg(required = true, add = all_branch_completer())]
  pub branch: String,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// List all worktrees for a repository
#[derive(Args)]
pub struct ListCommand {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Clean up stale worktrees
#[derive(Args)]
pub struct CleanCommand {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

pub(crate) fn handle_worktree_command(worktree: WorktreeArgs) -> Result<()> {
  match worktree.subcommand {
    WorktreeSubcommands::Clean(clean) => {
      let repo_path = detect_repository_from_path(clean.repo.as_deref().unwrap_or("."))
        .ok_or_else(|| anyhow::anyhow!("Could not detect repository path"))?;
      clean_worktrees(repo_path)
    }
    WorktreeSubcommands::Create(create) => {
      let repo_path = detect_repository_from_path(create.repo.as_deref().unwrap_or("."))
        .ok_or_else(|| anyhow::anyhow!("Could not detect repository path"))?;
      create_worktree(repo_path, &create.branch)?;
      Ok(())
    }
    WorktreeSubcommands::List(list) => {
      let repo_path = detect_repository_from_path(list.repo.as_deref().unwrap_or("."))
        .ok_or_else(|| anyhow::anyhow!("Could not detect repository path"))?;
      list_worktrees(repo_path)
    }
  }
}

/// List all worktrees for a repository
pub fn list_worktrees<P: AsRef<Path>>(repo_path: P) -> Result<()> {
  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the list of worktrees from git
  let worktree_names = repo.worktrees()?;

  if worktree_names.is_empty() {
    print_warning("No worktrees found for this repository.");
    println!(
      "Create one with {}",
      format_command("twig worktree create <branch-name>")
    );
    return Ok(());
  }

  // Load the repository state to get additional metadata
  let state = RepoState::load(repo_path)?;

  print_header("Worktrees");

  // Get all worktrees from the state
  let state_worktrees = state.list_worktrees();

  // Iterate through the worktree names
  for i in 0..worktree_names.len() {
    if let Some(name) = worktree_names.get(i) {
      let worktree = repo.find_worktree(name)?;
      let path = worktree.path().to_string_lossy().to_string();

      // Try to get additional metadata from the state
      let state_worktree = state.get_worktree(name);

      println!("  Branch: {name}",);
      println!("  Path: {}", format_repo_path(&path));

      if let Some(wt) = state_worktree {
        println!("  Created: {}", format_timestamp(&wt.created_at));
      } else {
        // If we don't have metadata in the state, check if we have any worktrees in the
        // state
        if !state_worktrees.is_empty() {
          println!("  Created: Unknown (no metadata available)");
        }
      }

      println!();
    }
  }

  Ok(())
}

/// Clean up stale worktrees
fn clean_worktrees<P: AsRef<Path>>(repo_path: P) -> Result<()> {
  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the list of worktrees from git
  let worktree_names = repo.worktrees()?;

  if worktree_names.is_empty() {
    print_warning("No worktrees found for this repository.");
    return Ok(());
  }

  // Load the repository state
  let mut state = RepoState::load(repo_path)?;
  let mut cleaned_count = 0;

  // Iterate through the worktree names
  for i in 0..worktree_names.len() {
    if let Some(name) = worktree_names.get(i) {
      let worktree = repo.find_worktree(name)?;
      let path = worktree.path();

      // Check if the worktree directory still exists
      if !path.exists() {
        println!("Cleaning up stale worktree reference: {name} (path no longer exists)",);

        // Prune the worktree reference
        worktree.prune(None)?;

        // Remove from state
        state.remove_worktree(name);

        cleaned_count += 1;
      }
    }
  }

  // Save the updated state
  state.save(repo_path)?;

  if cleaned_count > 0 {
    print_success(&format!("Cleaned up {cleaned_count} stale worktree references"));
  } else {
    println!("No stale worktrees found to clean up");
  }

  Ok(())
}
