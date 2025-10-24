//! MCP Server implementation with stdio transport

use crate::protocol::*;
use crate::{resources, tools};
use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "twig-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct McpServer {
    initialized: bool,
}

impl McpServer {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Run the MCP server on stdio
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting Twig MCP Server v{}", SERVER_VERSION);
        
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .await
                .context("Failed to read from stdin")?;

            if bytes_read == 0 {
                info!("EOF received, shutting down");
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            debug!("Received message: {}", trimmed);

            let response = match self.handle_message(trimmed).await {
                Ok(response) => response,
                Err(e) => {
                    error!("Error handling message: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError::internal_error(e.to_string())),
                    }
                }
            };

            let response_json = serde_json::to_string(&response)
                .context("Failed to serialize response")?;
            
            debug!("Sending response: {}", response_json);
            
            stdout
                .write_all(response_json.as_bytes())
                .await
                .context("Failed to write to stdout")?;
            stdout
                .write_all(b"\n")
                .await
                .context("Failed to write newline to stdout")?;
            stdout.flush().await.context("Failed to flush stdout")?;
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: &str) -> Result<JsonRpcResponse> {
        let request: JsonRpcRequest = serde_json::from_str(message)
            .context("Failed to parse JSON-RPC request")?;

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await?,
            "initialized" => {
                // Just acknowledge
                Value::Null
            }
            "tools/list" => self.handle_list_tools().await?,
            "tools/call" => self.handle_call_tool(request.params).await?,
            "resources/list" => self.handle_list_resources().await?,
            "resources/read" => self.handle_read_resource(request.params).await?,
            "ping" => {
                info!("Ping received");
                serde_json::json!({})
            }
            method => {
                warn!("Unknown method: {}", method);
                return Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError::method_not_found(format!(
                        "Unknown method: {}",
                        method
                    ))),
                });
            }
        };

        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(result),
            error: None,
        })
    }

    async fn handle_initialize(&mut self, params: Option<Value>) -> Result<Value> {
        let _params: InitializeParams = serde_json::from_value(
            params.ok_or_else(|| anyhow::anyhow!("Missing initialize params"))?
        ).context("Invalid initialize params")?;

        self.initialized = true;

        let result = InitializeResult {
            protocol_version: PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: ToolsCapability {
                    list_changed: false,
                },
                resources: ResourcesCapability {
                    subscribe: false,
                    list_changed: true,
                },
            },
            server_info: ServerInfo {
                name: SERVER_NAME.to_string(),
                version: SERVER_VERSION.to_string(),
            },
        };

        info!("Server initialized");
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_list_tools(&self) -> Result<Value> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Server not initialized"));
        }

        let tools = tools::get_tools();
        let result = ListToolsResult { tools };
        
        debug!("Listed {} tools", result.tools.len());
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_call_tool(&self, params: Option<Value>) -> Result<Value> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Server not initialized"));
        }

        let call_params: CallToolParams = serde_json::from_value(
            params.ok_or_else(|| anyhow::anyhow!("Missing tool call params"))?
        ).context("Invalid tool call params")?;

        info!("Calling tool: {}", call_params.name);
        
        let result = tools::call_tool(call_params).await?;
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_list_resources(&self) -> Result<Value> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Server not initialized"));
        }

        let result = resources::list_resources()?;
        debug!("Listed {} resources", result.resources.len());
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_read_resource(&self, params: Option<Value>) -> Result<Value> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Server not initialized"));
        }

        let read_params: ReadResourceParams = serde_json::from_value(
            params.ok_or_else(|| anyhow::anyhow!("Missing resource read params"))?
        ).context("Invalid resource read params")?;

        info!("Reading resource: {}", read_params.uri);
        
        let result = resources::read_resource(read_params).await?;
        Ok(serde_json::to_value(result)?)
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
