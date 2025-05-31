use anyhow::Result;
use clap::Command;

// Import the derive-based implementation
use super::derive;

/// Build the git subcommand
pub fn build_command() -> Command {
  derive::git::GitCommand::command()
}

/// Handle git subcommands
pub fn handle_commands(git_matches: &clap::ArgMatches) -> Result<()> {
  derive::git::GitCommand::parse_and_execute(git_matches)
}
