use anyhow::Result;
use no_worries::{Config as NoWorriesConfig, Metadata as NoWorriesMetadata, no_worries};

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
  // Set up the no-worries panic handler with custom configuration
  let config: NoWorriesConfig = NoWorriesConfig {
    metadata: NoWorriesMetadata {
      name: "twig".to_string(),
      support_email: Some("ejones@lat.ai".to_string()),
      // Other metadata fields use defaults from Cargo.toml
      ..Default::default()
    },
    ..Default::default()
  };
  no_worries!(config).expect("Failed to set up panic handler");

  let matches = cli::build_cli().get_matches();
  cli::handle_commands(&matches)
}
