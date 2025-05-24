use anyhow::Result;
use clap::Command;

mod cli;
mod config;
mod git;
mod state;
mod utils;

fn main() -> Result<()> {
  let matches = build_cli().get_matches();

  match matches.subcommand() {
    Some(("version", _)) => {
      print_version();
      Ok(())
    }
    _ => {
      println!("No command specified. Use --help for usage information.");
      Ok(())
    }
  }
}

fn build_cli() -> Command {
  Command::new("twig")
    .about("A Git-based developer productivity tool")
    .version(env!("CARGO_PKG_VERSION"))
    .subcommand_required(false)
    .subcommand(Command::new("version").about("Display version information"))
  // Additional commands will be added in future iterations
}

fn print_version() {
  let version = env!("CARGO_PKG_VERSION");
  let rust_version = env!("CARGO_PKG_RUST_VERSION");

  println!("twig version {}", version);
  println!("Minimum supported Rust version: {}", rust_version);
  println!("Dependencies:");
  println!("  clap: 4.x");
  println!("  git2: 0.18.x");
  println!("  tokio: 1.x");
}
