//! # Init Command
//!
//! Derive-based implementation of the init command for initializing twig
//! configuration.

use anyhow::Result;
use clap::{CommandFactory, Parser};

use super::DeriveCommand;

/// Command for initializing twig configuration
#[derive(Parser)]
#[command(name = "init")]
#[command(about = "Initialize twig configuration")]
#[command(long_about = "Initializes the twig configuration for your environment.\n\n\
            This creates necessary configuration files in your home directory to track\n\
            repositories and store settings. Run this command once before using other\n\
            twig features. No credentials are required for this operation.")]
pub struct InitCommand {}

impl DeriveCommand for InitCommand {
  fn execute(self) -> Result<()> {
    crate::config::init()
  }
}

impl InitCommand {
  /// Creates a clap Command for this command (for backward compatibility)
  pub fn command() -> clap::Command {
    Self::command_for_update()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute() -> Result<()> {
    let _cmd = Self::parse();
    Self::execute()
  }

  /// Executes the init command
  pub fn execute() -> Result<()> {
    crate::config::init()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn verify_cli() {
    InitCommand::command().debug_assert();
  }
}
