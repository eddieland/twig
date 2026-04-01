//! # Twig CLI Entry Point
//!
//! The main entry point for the twig command-line tool, a Git-based developer
//! productivity tool for managing branch dependencies and workflows.

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
use human_panic::setup_panic;
use tracing::debug;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};
use twig_cli::cli::{self, handle_cli};

fn main() -> Result<()> {
  setup_panic!();

  // Handle shell completion via CompleteEnv (activated by COMPLETE=<shell> env var)
  CompleteEnv::with_factory(cli::Cli::command).complete();

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
