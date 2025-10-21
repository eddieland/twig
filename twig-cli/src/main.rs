//! # Twig CLI Entry Point
//!
//! The main entry point for the twig command-line tool, a Git-based developer
//! productivity tool for managing branch dependencies and workflows.

use anyhow::Result;
use clap::Parser;
use cli::handle_cli;
use no_worries::{Config as NoWorriesConfig, Metadata as NoWorriesMetadata, no_worries};
use tracing::debug;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

mod auto_dependency_discovery;
mod cli;
mod clients;
mod completion;
mod consts;
mod creds;
mod diagnostics;
mod fixup;
mod git;
mod plugin;
mod self_update;
mod user_defined_dependency_resolver;
mod utils;

fn main() -> Result<()> {
  // Set up the no-worries panic handler with custom configuration
  let config: NoWorriesConfig = NoWorriesConfig {
    metadata: NoWorriesMetadata {
      name: "twig".to_string(),
      support_email: Some("e@eddie.land".to_string()),
      // Other metadata fields use defaults from Cargo.toml
      ..Default::default()
    },
    ..Default::default()
  };
  no_worries!(config).expect("Failed to set up panic handler");

  // Parse CLI arguments using the derive-based implementation
  let cmd = cli::Cli::parse();

  // Set up tracing based on verbosity level
  let verbose_count = cmd.verbose;
  let level = match verbose_count {
    0 => tracing::Level::WARN,  // Default: warnings and errors
    1 => tracing::Level::INFO,  // -v: info, warnings, and errors
    2 => tracing::Level::DEBUG, // -vv: debug, info, warnings, and errors
    _ => tracing::Level::TRACE, // -vvv or more: trace and everything else
  };

  // Initialize the tracing subscriber with the specified level
  tracing_subscriber::registry()
    .with(fmt::layer())
    .with(EnvFilter::from_default_env().add_directive(level.into()))
    .init();

  debug!("Tracing initialized with level: {}", level);

  handle_cli(cmd)
}
