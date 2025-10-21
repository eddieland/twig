//! # Self Command
//!
//! Implements the `twig self` command group for managing Twig's
//! own lifecycle behaviours like self-updating.

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::self_update::{SelfUpdateOptions, run as run_self_update};

/// Arguments for the top-level `twig self` command.
#[derive(Args, Debug)]
pub struct SelfArgs {
  /// Subcommands under `twig self`
  #[command(subcommand)]
  pub command: SelfSubcommand,
}

/// Subcommands available under `twig self`.
#[derive(Subcommand, Debug)]
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
  }
}
