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

/// Handle unknown or missing commands
pub fn handle_unknown_command() -> Result<()> {
  use crate::utils::output::print_info;
  print_info("No command specified.");
  // Print the help text directly instead of telling the user to use --help
  let mut cmd = crate::cli::build_cli();
  cmd.print_help().expect("Failed to print help text");
  println!();
  Ok(())
}
