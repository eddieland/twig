use anyhow::Result;
use clap::Command;

// Import the derive-based implementations
use super::derive;

/// Build the init subcommand
pub fn build_init_command() -> Command {
  derive::init::InitCommand::command()
}

/// Handle the init command
pub fn handle_init_command() -> Result<()> {
  derive::init::InitCommand::parse_and_execute()
}

/// Build the panic test subcommand
pub fn build_panic_command() -> Command {
  derive::panic::PanicCommand::command()
}

/// Handle the panic test command - intentionally panics
pub fn handle_panic_command() -> Result<()> {
  derive::panic::PanicCommand::parse_and_execute()
}
