//! # Sync Command
//!
//! CLI commands for synchronizing branch metadata with external services,
//! automatically detecting and linking issues from branch names and commit
//! messages.

use anyhow::Result;
use clap::Command;

// Import the derive-based implementation
use super::derive;

/// Build the sync subcommand
pub fn build_command() -> Command {
  derive::sync::SyncCommand::command()
}

/// Handle the sync command
pub fn handle_command(sync_matches: &clap::ArgMatches) -> Result<()> {
  derive::sync::SyncCommand::parse_and_execute(sync_matches)
}
