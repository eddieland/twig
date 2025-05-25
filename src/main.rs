use anyhow::Result;

mod api;
mod cli;
mod config;
mod creds;
mod git;
mod state;
mod utils;
mod worktree;

fn main() -> Result<()> {
  let matches = cli::build_cli().get_matches();
  cli::handle_commands(&matches)
}
