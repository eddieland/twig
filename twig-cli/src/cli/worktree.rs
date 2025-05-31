//! # Worktree Command
//!
//! CLI commands for managing Git worktrees for efficient multi-branch
//! development.

use anyhow::Result;
use clap::Command;

// Import the derive-based implementation
use super::derive;

/// Build the worktree subcommand
pub fn build_command() -> Command {
  derive::worktree::WorktreeCommand::command()
}

/// Handle worktree subcommands
pub fn handle_commands(worktree_matches: &clap::ArgMatches) -> Result<()> {
  derive::worktree::WorktreeCommand::parse_and_execute(worktree_matches)
}
