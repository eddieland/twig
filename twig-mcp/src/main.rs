//! twig-mcp: MCP server exposing twig branch metadata, Jira issues, and GitHub PRs.

mod context;
mod server;
mod tools;
mod types;

use anyhow::{Context, Result};
use directories::BaseDirs;
use rmcp::ServiceExt;
use twig_core::config::ConfigDirs;
use twig_core::git::detection::detect_repository;

use crate::context::ServerContext;
use crate::server::TwigMcpServer;

#[tokio::main]
async fn main() -> Result<()> {
  // Tracing to stderr — stdout is reserved for MCP JSON-RPC protocol.
  tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_env_filter("twig_mcp=info")
    .init();

  let config_dirs = ConfigDirs::new().context("Failed to initialise config directories")?;
  let repo_path = detect_repository();
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
