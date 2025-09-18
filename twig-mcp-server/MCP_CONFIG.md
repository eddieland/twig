## Twig MCP Server Configuration Example

Here's how to configure the Twig MCP server with different MCP clients:

### Claude Desktop Configuration

Add to your Claude Desktop `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "twig": {
      "command": "node",
      "args": ["/path/to/twig-mcp-server/dist/index.js"],
      "cwd": "/path/to/your/git/repository",
      "env": {
        "PATH": "/usr/local/bin:/usr/bin:/bin"
      }
    }
  }
}
```

### VS Code Extension Configuration

For VS Code extensions that support MCP:

```json
{
  "mcp.servers": {
    "twig": {
      "command": ["node", "/path/to/twig-mcp-server/dist/index.js"],
      "args": [],
      "cwd": "${workspaceFolder}"
    }
  }
}
```

### Environment Variables

The server may need these environment variables:
- `GITHUB_TOKEN` - For GitHub PR creation
- `JIRA_HOST` - For Jira integration (if used)
- `PATH` - Must include git and twig CLI tools

### Testing the Server

You can test the server directly using the MCP protocol:

```bash
cd /path/to/your/git/repo
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | node /path/to/twig-mcp-server/dist/index.js
```

This should return a list of available tools.