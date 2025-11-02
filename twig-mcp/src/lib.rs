//! # Twig MCP Server
//!
//! Model Context Protocol server implementation for twig, enabling LLM
//! integration for Git branch tree management, worktree operations, and
//! dependency tracking.
//!
//! This server exposes twig's functionality through the MCP protocol, allowing
//! AI assistants in VS Code and other tools to interact with Git repositories
//! using twig's branch tree abstraction.

pub mod protocol;
pub mod resources;
pub mod server;
pub mod tools;

pub use server::McpServer;
