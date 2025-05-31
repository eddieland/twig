//! # Branch Command
//!
//! CLI commands for managing branch dependencies and root branches,
//! including adding, removing, and listing branch relationships.

use anyhow::Result;
use clap::Command;

// Import the derive-based implementation
use super::derive;

/// Build the branch command
pub fn build_command() -> Command {
  derive::branch::BranchCommand::command()
}

/// Handle branch commands
pub fn handle_commands(branch_matches: &clap::ArgMatches) -> Result<()> {
  derive::branch::BranchCommand::parse_and_execute(branch_matches)
}
