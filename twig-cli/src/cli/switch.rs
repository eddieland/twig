//! # Switch Command
//!
//! CLI command for intelligently switching to branches based on various inputs.

use anyhow::Result;
use clap::Command;

// Import the derive-based implementation
use super::derive;

/// Build the switch subcommand
pub fn build_command() -> Command {
  derive::switch::SwitchCommand::command()
}

/// Handle the switch command
pub fn handle_command(switch_matches: &clap::ArgMatches) -> Result<()> {
  derive::switch::SwitchCommand::parse_and_execute(switch_matches)
}
