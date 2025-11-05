//! # Self Command
//!
//! Implements the `twig self` command group for managing Twig's
//! own lifecycle behaviours like self-updating.

use anyhow::Result;
use clap::{Args, Subcommand};

use super::completion;
use crate::diagnostics;
use crate::self_update::{SelfUpdateOptions, run as run_self_update};

/// Arguments for the top-level `twig self` command.
#[derive(Args)]
pub struct SelfArgs {
  /// Subcommands under `twig self`
  #[command(subcommand)]
  pub command: SelfSubcommand,
}

/// Subcommands available under `twig self`.
#[derive(Subcommand)]
pub enum SelfSubcommand {
  /// Update the Twig binary to the latest release
  #[command(
    long_about = "Download the latest Twig release from GitHub and replace the current executable.\n\n\
This command determines the platform-specific binary to download, verifies permissions,\
handles sudo elevation when required, and ensures that the running executable is swapped\
out safely once the update completes."
  )]
  #[command(alias = "upgrade")]
  Update(SelfUpdateCommand),

  /// Run system diagnostics
  #[command(
    long_about = "Runs comprehensive system diagnostics to check twig's configuration and dependencies.\n\n\
            This command checks system information, configuration directories, credentials,\n\
            git configuration, tracked repositories, and network connectivity. Use this\n\
            command to troubleshoot issues or verify that twig is properly configured."
  )]
  #[command(alias = "diag")]
  Diagnose,

  /// Generate shell completions
  #[command(long_about = "Generates shell completion scripts for twig commands.\n\n\
            This command generates completion scripts that provide tab completion for twig\n\
            commands and options in your shell. Supported shells include bash, zsh, and fish.")]
  Completion(completion::CompletionArgs),
}

/// Options for `twig self update`.
#[derive(Args, Debug, Clone)]
pub struct SelfUpdateCommand {
  /// Reinstall even if the latest version is already installed
  #[arg(long)]
  pub force: bool,
}

impl From<SelfUpdateCommand> for SelfUpdateOptions {
  fn from(value: SelfUpdateCommand) -> Self {
    SelfUpdateOptions { force: value.force }
  }
}

/// Execute a `twig self` command.
pub fn handle_self_command(args: SelfArgs) -> Result<()> {
  match args.command {
    SelfSubcommand::Update(cmd) => run_self_update(cmd.into()),
    SelfSubcommand::Diagnose => diagnostics::run_diagnostics(),
    SelfSubcommand::Completion(cmd) => completion::handle_completion_command(cmd),
  }
}
