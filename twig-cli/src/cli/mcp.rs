//! MCP Server command handler

use anyhow::Result;
use tracing::info;

/// Handle the MCP server command
pub fn handle_mcp_server_command() -> Result<()> {
    info!("Starting Twig MCP Server");
    
    // Create a Tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;
    
    runtime.block_on(async {
        let mut server = twig_mcp::McpServer::new();
        server.run().await
    })
}
