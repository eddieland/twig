use anyhow::Result;
use clap::Command;

/// Build the init subcommand
pub fn build_init_command() -> Command {
  Command::new("init").about("Initialize twig configuration").long_about(
    "Initializes the twig configuration for your environment.\n\n\
            This creates necessary configuration files in your home directory to track\n\
            repositories and store settings. Run this command once before using other\n\
            twig features. No credentials are required for this operation.",
  )
}

/// Handle the init command
pub fn handle_init_command() -> Result<()> {
  crate::config::init()
}

/// Build the panic test subcommand
pub fn build_panic_command() -> Command {
  Command::new("panic")
    .about("Test the panic handler")
    .long_about(
      "TEMPORARY COMMAND: Intentionally triggers a panic to test the no-worries panic handler.\n\n\
            This command is for testing purposes only and will be removed in a future version.",
    )
    .hide(true)
}

/// Handle the panic test command - intentionally panics
pub fn handle_panic_command() -> Result<()> {
  panic!("This is an intentional test panic to verify no-worries integration");
}
