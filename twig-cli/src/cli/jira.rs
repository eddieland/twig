//! # Jira Command
//!
//! CLI commands for Jira integration, including issue management, transitions,
//! and synchronization with branch metadata for workflow automation.

use anyhow::Result;
use clap::Command;

use crate::cli::derive::jira::JiraCommand;

/// Build the jira subcommand
pub fn build_command() -> Command {
  JiraCommand::command()
}

/// Handle jira subcommands
pub fn handle_commands(jira_matches: &clap::ArgMatches) -> Result<()> {
  JiraCommand::parse_and_execute(jira_matches)
}
