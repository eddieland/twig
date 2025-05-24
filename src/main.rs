use anyhow::Result;

mod cli;
mod config;
mod git;
mod state;
mod utils;

fn main() -> Result<()> {
  let matches = cli::build_cli().get_matches();
  cli::handle_commands(&matches)
}
