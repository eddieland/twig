//! # GitHub Command
//!
//! CLI commands for GitHub integration, including pull request management,
//! status checks, and synchronization with branch metadata for development
//! workflows.

use anyhow::Result;
use clap::Command;

use crate::cli::derive::github::GitHubCommand;

/// Build the GitHub command
pub fn build_command() -> Command {
  GitHubCommand::command()
}

/// Handle GitHub commands
pub fn handle_commands(github_matches: &clap::ArgMatches) -> Result<()> {
  GitHubCommand::parse_and_execute(github_matches)
}
