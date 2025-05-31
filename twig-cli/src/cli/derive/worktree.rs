//! # Worktree Command
//!
//! Derive-based implementation of the worktree command for managing Git
//! worktrees for efficient multi-branch development.

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

use crate::cli::derive::DeriveCommand;
use crate::utils::output::print_warning;

/// Command for worktree management
#[derive(Parser)]
#[command(name = "worktree")]
#[command(about = "Worktree management")]
#[command(long_about = "Manage Git worktrees for efficient multi-branch development.\n\n\
            Worktrees allow you to check out multiple branches simultaneously in separate\n\
            directories, all connected to the same repository. This enables working on\n\
            different features or fixes concurrently without stashing or committing\n\
            incomplete work.")]
#[command(alias = "wt")]
pub struct WorktreeCommand {
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
#[derive(Parser)]
pub struct CreateCommand {
  /// Branch name
  #[arg(required = true)]
  pub branch: String,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// List all worktrees for a repository
#[derive(Parser)]
pub struct ListCommand {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Clean up stale worktrees
#[derive(Parser)]
pub struct CleanCommand {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

impl WorktreeCommand {
  /// Creates a clap Command for this command (for backward compatibility)
  pub fn command() -> clap::Command {
    <Self as CommandFactory>::command()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
      Some(("create", create_matches)) => {
        let branch = create_matches.get_one::<String>("branch").unwrap().clone();
        let repo = create_matches.get_one::<String>("repo").cloned();

        let cmd = Self {
          subcommand: WorktreeSubcommands::Create(CreateCommand { branch, repo }),
        };
        cmd.execute()
      }
      Some(("list", list_matches)) => {
        let repo = list_matches.get_one::<String>("repo").cloned();

        let cmd = Self {
          subcommand: WorktreeSubcommands::List(ListCommand { repo }),
        };
        cmd.execute()
      }
      Some(("clean", clean_matches)) => {
        let repo = clean_matches.get_one::<String>("repo").cloned();

        let cmd = Self {
          subcommand: WorktreeSubcommands::Clean(CleanCommand { repo }),
        };
        cmd.execute()
      }
      _ => {
        print_warning("Unknown worktree command.");
        Ok(())
      }
    }
  }
}

impl DeriveCommand for WorktreeCommand {
  fn execute(self) -> Result<()> {
    match self.subcommand {
      WorktreeSubcommands::Create(cmd) => {
        let repo_path = crate::utils::resolve_repository_path(cmd.repo.as_deref())?;
        crate::repo_state::create_worktree(repo_path, &cmd.branch)?;
        Ok(())
      }
      WorktreeSubcommands::List(cmd) => {
        let repo_path = crate::utils::resolve_repository_path(cmd.repo.as_deref())?;
        crate::repo_state::list_worktrees(repo_path)
      }
      WorktreeSubcommands::Clean(cmd) => {
        let repo_path = crate::utils::resolve_repository_path(cmd.repo.as_deref())?;
        crate::repo_state::clean_worktrees(repo_path)
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use clap::CommandFactory;

  use super::*;

  #[test]
  fn verify_cli() {
    WorktreeCommand::command().debug_assert();
  }
}
