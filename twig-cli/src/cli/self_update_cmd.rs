//! Self-update command

use anyhow::Result;
use clap::Args;

use crate::self_update;

/// Update the Twig binary to the latest release
#[derive(Debug, Clone, Args, Default)]
pub struct SelfUpdateArgs {}

/// Execute the self-update workflow
pub fn handle_self_update_command(_args: SelfUpdateArgs) -> Result<()> {
  self_update::run_self_update()
}
