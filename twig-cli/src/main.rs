//! # Twig CLI Entry Point
//!
//! The main entry point for the twig command-line tool, a Git-based developer
//! productivity tool for managing branch dependencies and workflows.

use anyhow::Result;
use no_worries::{Config as NoWorriesConfig, Metadata as NoWorriesMetadata, no_worries};

mod auto_dependency_discovery;
mod cli;
mod completion;
mod config;
mod creds;
mod diagnostics;
mod git;
mod repo_state;
mod state;
mod tree_renderer;
mod user_defined_dependency_resolver;
mod utils;

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
