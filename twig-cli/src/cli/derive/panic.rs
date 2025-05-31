//! # Panic Command
//!
//! Derive-based implementation of the panic command for testing the panic
//! handler.

use anyhow::Result;
use clap::{CommandFactory, Parser};

use super::DeriveCommand;

/// Command for testing the panic handler
#[derive(Parser)]
#[command(name = "panic")]
#[command(about = "Test the panic handler")]
#[command(
  long_about = "TEMPORARY COMMAND: Intentionally triggers a panic to test the no-worries panic handler.\n\n\
            This command is for testing purposes only and will be removed in a future version."
)]
#[command(hide = true)]
pub struct PanicCommand {}

impl DeriveCommand for PanicCommand {
  fn execute(self) -> Result<()> {
    panic!("This is an intentional test panic to verify no-worries integration");
  }
}

impl PanicCommand {
  /// Creates a clap Command for this command (for backward compatibility)
  pub fn command() -> clap::Command {
    Self::command_for_update()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute() -> Result<()> {
    let _cmd = Self::parse();
    Self::execute()
  }

  /// Executes the panic command - intentionally panics
  pub fn execute() -> Result<()> {
    panic!("This is an intentional test panic to verify no-worries integration");
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn verify_cli() {
    PanicCommand::command().debug_assert();
  }
}
