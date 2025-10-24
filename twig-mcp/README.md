# Twig MCP Server

The **Twig MCP Server** enables AI assistants like GitHub Copilot in VS Code to interact with your Git repository using twig's branch tree management capabilities through the Model Context Protocol (MCP).

## Features

The MCP server exposes the following capabilities:

### Tools (Actions)

- **`twig_list_branches`** - List all branches with their metadata (Jira issues, GitHub PRs)
- **`twig_get_branch_tree`** - Get the branch dependency tree visualization
- **`twig_get_branch_info`** - Get detailed information about a specific branch
- **`twig_get_worktrees`** - List all Git worktrees in the repository
- **`twig_get_registry`** - Get all repositories registered with twig

### Resources (Data Access)

- **`twig://registry`** - Global registry of all twig-managed repositories
- **`twig://repo/{name}/state`** - Branch metadata and state for a specific repository
- **`twig://repo/{name}/tree`** - Branch dependency tree for a specific repository

## Installation

### Prerequisites

1. Ensure `twig` is built and available in your PATH:
   ```bash
   cargo build --release
   # Add target/release to your PATH or install globally
   cargo install --path .
   ```

2. Have VS Code with GitHub Copilot installed

### VS Code Configuration

Add the following to your VS Code settings (`.vscode/settings.json` in your workspace or user settings):

```json
{
  "github.copilot.chat.mcp.servers": {
    "twig": {
      "command": "twig",
      "args": ["mcp-server"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

### Verify Installation

1. Restart VS Code or reload the window
2. Open GitHub Copilot Chat
3. The twig MCP server should automatically start when you interact with Copilot
4. Try asking: "What branches do I have in this repository?"

## Usage Examples

Once configured, you can interact with your Git repository through natural language in Copilot Chat:

### Branch Management

- "What branches do I have?"
- "Show me the branch tree for this repository"
- "What's the status of the feature/login branch?"
- "List all my worktrees"

### Repository Information

- "Which repositories are registered with twig?"
- "Show me all branches with their Jira issues"
- "What GitHub PRs are linked to my branches?"

### Dependency Tracking

- "Show me the dependency tree for my branches"
- "What branches depend on the main branch?"

## How It Works

The MCP server:

1. **Listens on stdio** - Communicates with VS Code via standard input/output
2. **Speaks JSON-RPC** - Uses the JSON-RPC 2.0 protocol for message exchange
3. **Provides tools** - Exposes twig commands as callable tools for the LLM
4. **Shares resources** - Makes repository state available as queryable resources
5. **Runs async** - Uses Tokio for efficient async I/O operations

## Architecture

```
┌─────────────────────┐
│   VS Code           │
│   GitHub Copilot    │
└──────────┬──────────┘
           │ JSON-RPC over stdio
           ▼
┌─────────────────────┐
│  Twig MCP Server    │
│  (twig mcp-server)  │
├─────────────────────┤
│  • Protocol Handler │
│  • Tools            │
│  • Resources        │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Twig Core          │
│  • State            │
│  • Config           │
│  • Git Operations   │
└─────────────────────┘
```

## Troubleshooting

### Server Not Starting

Check the MCP server logs in VS Code:
1. Open Output panel (View → Output)
2. Select "GitHub Copilot Chat" from the dropdown
3. Look for twig-related messages

### Enable Debug Logging

Update your settings to include verbose logging:

```json
{
  "github.copilot.chat.mcp.servers": {
    "twig": {
      "command": "twig",
      "args": ["mcp-server"],
      "env": {
        "RUST_LOG": "debug,twig_mcp=trace"
      }
    }
  }
}
```

### Test Manually

You can test the MCP server directly:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | twig mcp-server
```

## Development

### Running Tests

```bash
cargo test -p twig-mcp
```

### Building

```bash
cargo build -p twig-mcp
```

### Adding New Tools

1. Add tool definition to `twig-mcp/src/tools.rs::get_tools()`
2. Implement handler in `tools::call_tool()`
3. Use existing twig-core APIs for functionality
4. Rebuild and restart VS Code

### Adding New Resources

1. Add resource to `twig-mcp/src/resources.rs::list_resources()`
2. Implement handler in `resources::read_resource()`
3. Return data as JSON

## Protocol Version

This implementation follows the Model Context Protocol **2024-11-05** specification.

## License

Same as the main twig project (MIT License).
