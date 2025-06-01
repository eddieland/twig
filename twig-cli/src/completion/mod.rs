//! # Shell Completion
//!
//! Generates shell completion scripts for various shells (bash, zsh, fish,
//! etc.) to provide tab completion for twig commands and arguments.

use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::Cli;
use crate::utils::output::print_error;

/// Generate shell completions for the specified shell
pub fn generate_completions(shell: Shell) -> Result<()> {
  let mut cmd = Cli::command();
  let app_name = cmd.get_name().to_string();

  generate(shell, &mut cmd, app_name, &mut io::stdout());

  Ok(())
}

/// Parse a shell string into a Shell enum
pub fn parse_shell(shell_str: &str) -> Result<Shell> {
  match shell_str.to_lowercase().as_str() {
    "bash" => Ok(Shell::Bash),
    "zsh" => Ok(Shell::Zsh),
    "fish" => Ok(Shell::Fish),
    _ => {
      print_error(&format!("Unsupported shell: {shell_str}",));
      println!("Supported shells: bash, zsh, fish");
      Err(anyhow::anyhow!("Unsupported shell: {}", shell_str))
    }
  }
}
