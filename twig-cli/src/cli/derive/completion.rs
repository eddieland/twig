//! # Completion Command
//!
//! Derive-based implementation of the completion command for generating
//! shell completion scripts.

use anyhow::Result;
use clap::{CommandFactory, Parser};

use crate::cli::derive::DeriveCommand;
use crate::completion::{generate_completions, parse_shell};

/// Command for generating shell completions
#[derive(Parser)]
#[command(name = "completion")]
#[command(about = "Generate shell completions")]
#[command(long_about = "Generates shell completion scripts for twig commands.\n\n\
            This command generates completion scripts that provide tab completion for twig\n\
            commands and options in your shell. Supported shells include bash, zsh, and fish.")]
pub struct CompletionCommand {
  /// Shell to generate completions for
  #[arg(required = true, value_parser = ["bash", "zsh", "fish"])]
  pub shell: String,
}

impl CompletionCommand {
  /// Creates a clap Command for this command
  pub fn command() -> clap::Command {
    <Self as CommandFactory>::command()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute(matches: &clap::ArgMatches) -> Result<()> {
    let shell = matches.get_one::<String>("shell").unwrap().clone();

    let cmd = Self { shell };
    cmd.execute()
  }
}

impl DeriveCommand for CompletionCommand {
  fn execute(self) -> Result<()> {
    let shell = parse_shell(&self.shell)?;
    generate_completions(shell)
  }
}

#[cfg(test)]
mod tests {
  use clap::CommandFactory;

  use super::*;

  #[test]
  fn verify_cli() {
    CompletionCommand::command().debug_assert();
  }
}
