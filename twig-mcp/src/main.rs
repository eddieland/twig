//! twig-mcp: MCP server exposing twig branch metadata, Jira issues, and GitHub PRs.

mod context;
mod server;
mod tools;
mod types;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use directories::BaseDirs;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;
use twig_core::config::ConfigDirs;
use twig_core::git::detection::detect_repository;

use crate::context::ServerContext;
use crate::server::TwigMcpServer;

#[derive(Parser)]
#[command(version, about = "MCP server for twig branch metadata, Jira issues, and GitHub PRs")]
struct Cli {
  /// Sets the level of verbosity (can be used multiple times)
  #[arg(
    short = 'v',
    long = "verbose",
    action = ArgAction::Count,
    long_help = "Sets the level of verbosity for tracing and logging output.\n\n\
             -v: Show info level messages\n\
             -vv: Show debug level messages\n\
             -vvv: Show trace level messages"
  )]
  verbose: u8,

  /// Override repository path (defaults to auto-detection)
  #[arg(long = "repo", value_name = "PATH")]
  repo: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
  let cli = Cli::parse();

  // Tracing to stderr — stdout is reserved for MCP JSON-RPC protocol.
  let level = match cli.verbose {
    0 => tracing::Level::WARN,
    1 => tracing::Level::INFO,
    2 => tracing::Level::DEBUG,
    _ => tracing::Level::TRACE,
  };

  tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter(EnvFilter::from_default_env().add_directive(level.into()))
    .init();

  let config_dirs = ConfigDirs::new().context("Failed to initialise config directories")?;
  let repo_path = cli.repo.or_else(detect_repository);
  let home_dir = BaseDirs::new()
    .context("Failed to determine home directory")?
    .home_dir()
    .to_path_buf();

  let context = ServerContext::new(config_dirs, repo_path, home_dir);
  let server = TwigMcpServer::new(context);

  // Start MCP server on stdio
  let service = server.serve(rmcp::transport::io::stdio()).await?;
  service.waiting().await?;

  Ok(())
}
