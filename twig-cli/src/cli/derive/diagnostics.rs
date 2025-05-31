//! # Diagnostics Command
//!
//! Derive-based implementation of the diagnostics command for running
//! system diagnostics.

use anyhow::Result;
use clap::{CommandFactory, Parser};

use crate::cli::derive::DeriveCommand;
use crate::diagnostics::run_diagnostics;

/// Command for running system diagnostics
#[derive(Parser)]
#[command(name = "diagnose")]
#[command(about = "Run system diagnostics")]
#[command(alias = "diag")]
#[command(
  long_about = "Runs comprehensive system diagnostics to check twig's configuration and dependencies.\n\n\
            This command checks system information, configuration directories, credentials,\n\
            git configuration, tracked repositories, and network connectivity. Use this\n\
            command to troubleshoot issues or verify that twig is properly configured."
)]
pub struct DiagnosticsCommand {}

impl DiagnosticsCommand {
  /// Creates a clap Command for this command
  pub fn command() -> clap::Command {
    <Self as CommandFactory>::command()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute() -> Result<()> {
    let cmd = Self {};
    cmd.execute()
  }
}

impl DeriveCommand for DiagnosticsCommand {
  fn execute(self) -> Result<()> {
    run_diagnostics()
  }
}

#[cfg(test)]
mod tests {
  use clap::CommandFactory;

  use super::*;

  #[test]
  fn verify_cli() {
    DiagnosticsCommand::command().debug_assert();
  }
}
