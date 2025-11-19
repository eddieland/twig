mod cli;
mod switch;
mod tree;

use anyhow::Result;
use clap::Parser;
pub use cli::Cli;

/// Execute the plugin with the provided command-line arguments.
pub fn run() -> Result<()> {
  let cli = Cli::parse();

  if cli.target.is_some() {
    return switch::run(&cli);
  }

  tree::run(&cli)
}
