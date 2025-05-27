use anyhow::Result;

mod cli;
mod completion;
mod config;
mod creds;
mod diagnostics;
mod git;
mod state;
mod utils;
mod worktree;

fn main() -> Result<()> {
  let matches = cli::build_cli().get_matches();
  cli::handle_commands(&matches)
}
