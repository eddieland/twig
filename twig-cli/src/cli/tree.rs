//! # Tree Command
//!
//! CLI command for visualizing branch dependency trees, showing hierarchical
//! relationships between branches with optional depth limits and formatting
//! options.

use anyhow::Result;
use clap::Command;

// Import the derive-based implementation
use super::derive;

/// Build the tree subcommand
pub fn build_command() -> Command {
  derive::tree::TreeCommand::command()
}

/// Handle the tree command
pub fn handle_command(tree_matches: &clap::ArgMatches) -> Result<()> {
  derive::tree::TreeCommand::parse_and_execute(tree_matches)
}
