//! # Worktree Command
//!
//! Derive-based implementation of the worktree command for managing Git
//! worktrees for efficient multi-branch development.

use anyhow::Result;
use clap::{Args, Subcommand};

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

  /// Clean up stale worktrees
  #[command(
    long_about = "Removes worktrees that are no longer needed or have been abandoned.\n\n\
                     This helps keep your workspace clean and organized. The command checks for\n\
                     worktrees with branches that have been merged or deleted and offers to\n\
                     remove them. This operation only removes the worktree directories and\n\
                     doesn't affect the main repository."
  )]
  Clean(CleanCommand),
}

/// Create a new worktree for a branch
#[derive(Args)]
pub struct CreateCommand {
  /// Branch name
  #[arg(required = true)]
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
    WorktreeSubcommands::Create(create) => {
      let repo_path = crate::utils::resolve_repository_path(create.repo.as_deref())?;
      crate::repo_state::create_worktree(repo_path, &create.branch)?;
      Ok(())
    }
    WorktreeSubcommands::List(list) => {
      let repo_path = crate::utils::resolve_repository_path(list.repo.as_deref())?;
      crate::repo_state::list_worktrees(repo_path)
    }
    WorktreeSubcommands::Clean(clean) => {
      let repo_path = crate::utils::resolve_repository_path(clean.repo.as_deref())?;
      crate::repo_state::clean_worktrees(repo_path)
    }
  }
}
