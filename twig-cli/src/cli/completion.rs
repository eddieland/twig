//! # Completion Command
//!
//! Derive-based implementation of the completion command for generating
//! shell completion scripts.

use anyhow::Result;
use clap::Args;

use crate::completion::{generate_completions, parse_shell};

/// Command for generating shell completions
#[derive(Args)]
pub struct CompletionArgs {
  /// Shell to generate completions for
  #[arg(required = true, value_parser = ["bash", "zsh", "fish"])]
  pub shell: String,
}

/// Handle the completion command
///
/// This function takes the shell type as an argument and generates the
/// appropriate completion script for the specified shell.
pub(crate) fn handle_completion_command(completion: CompletionArgs) -> Result<()> {
  let shell = parse_shell(&completion.shell)?;
  generate_completions(shell)
}
