// Note: `complete` must be declared before `cli` since cli.rs imports from it
mod complete;

mod cli;
mod switch;
mod tree;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
pub use cli::Cli;

/// Execute the plugin with the provided command-line arguments.
pub fn run() -> Result<()> {
  // Handle shell completion via CompleteEnv (activated by COMPLETE=<shell> env var)
  CompleteEnv::with_factory(Cli::command).complete();

  let cli = Cli::parse();

  if cli.target.is_some() {
    return switch::run(&cli);
  }

  tree::run(&cli)
}
