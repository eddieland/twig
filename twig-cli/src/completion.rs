//! # Shell Completion
//!
//! Generates shell completion scripts for various shells (bash, zsh, fish,
//! PowerShell, etc.) to provide tab completion for twig commands and arguments.

use std::io;

use anyhow::Result;
use clap::{CommandFactory, ValueEnum};
use clap_complete::generate;

use crate::cli::Cli;

/// Shell with auto-generated completion script available.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Shell {
  /// Bourne Again `SHell` (bash)
  Bash,
  /// Friendly Interactive `SHell` (fish)
  Fish,
  /// `PowerShell`
  PowerShell,
  /// Z `SHell` (zsh)
  Zsh,
}

/// Generate shell completions for the specified shell
pub fn generate_completions(shell: clap_complete::Shell) -> Result<()> {
  let mut cmd = Cli::command();
  let app_name = cmd.get_name().to_string();

  generate(shell, &mut cmd, app_name, &mut io::stdout());

  Ok(())
}

#[cfg(test)]
mod tests {
  use clap_complete::Shell;

  use super::generate_completions;

  #[test]
  fn test_generate_completions_succeeds() {
    // Test that generating completions for each shell doesn't panic
    let shells = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];

    for shell in shells {
      let result = generate_completions(shell);
      assert!(result.is_ok(), "Failed to generate completions for {:?}", shell);
    }
  }
}
