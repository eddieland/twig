//! # Completion Command
//!
//! Derive-based implementation of the completion command for generating
//! shell completion scripts.

use anyhow::Result;
use clap::Args;

use crate::completion::{Shell, generate_completions};

/// Command for generating shell completions
#[derive(Args)]
pub struct CompletionArgs {
  /// Shell to generate completions for
  #[arg(value_enum)]
  pub shell: Shell,
}

/// Handle the completion command
///
/// This function takes the shell type as an argument and generates the
/// appropriate completion script for the specified shell.
pub(crate) fn handle_completion_command(completion: CompletionArgs) -> Result<()> {
  let shell = match completion.shell {
    Shell::Bash => clap_complete::Shell::Bash,
    Shell::Fish => clap_complete::Shell::Fish,
    Shell::PowerShell => clap_complete::Shell::PowerShell,
    Shell::Zsh => clap_complete::Shell::Zsh,
  };
  generate_completions(shell)?;

  Ok(())
}
