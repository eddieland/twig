use anyhow::Result;
use clap::Command;

use crate::diagnostics::run_diagnostics;

/// Build the diagnostics command
pub fn build_diagnostics_command() -> Command {
  Command::new("diagnose")
    .about("Run system diagnostics")
    .alias("diag")
    .long_about(
      "Runs comprehensive system diagnostics to check twig's configuration and dependencies.\n\n\
             This command checks system information, configuration directories, credentials,\n\
             git configuration, tracked repositories, and network connectivity. Use this\n\
             command to troubleshoot issues or verify that twig is properly configured.",
    )
}

/// Handle the diagnostics command
pub fn handle_diagnostics_command() -> Result<()> {
  run_diagnostics()
}
