mod cli;
mod tree;

use anyhow::Result;
use clap::Parser;
pub use cli::Cli;
use twig_core::output::print_info;

/// Execute the plugin with the provided command-line arguments.
pub fn run() -> Result<()> {
  let cli = Cli::parse();

  if cli.target.is_some() {
    print_info("Branch switching is not yet implemented for twig flow.");
    return Ok(());
  }

  tree::run(&cli)
}
