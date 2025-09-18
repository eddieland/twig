# Twig MCP Server

A Model Context Protocol (MCP) server for the twig git branch management tool. This server provides structured access to twig commands for AI assistants and other MCP clients.

## Features

- **Branch Tree Management**: Get structured branch tree information
- **Branch Operations**: Create, switch, delete branches with dependency tracking
- **Dependency Management**: Add/remove branch dependencies
- **Root Branch Management**: Manage root branches
- **Cleanup Operations**: Tidy and prune operations
- **GitHub Integration**: Create pull requests
- **Structured Data**: All responses are typed and structured (no CLI parsing needed)

## Installation

```bash
npm install
npm run build
```

## Usage

### As MCP Server

Add to your MCP client configuration:

```json
{
  "mcpServers": {
    "twig": {
      "command": "node",
      "args": ["path/to/twig-mcp-server/dist/index.js"],
      "cwd": "/path/to/your/git/repo"
    }
  }
}
```

### Available Tools

- `twig_get_tree` - Get branch tree structure
- `twig_switch_branch` - Switch to a branch
- `twig_create_branch` - Create new branch with dependencies
- `twig_delete_branch` - Delete a branch
- `twig_add_dependency` - Add branch dependency
- `twig_remove_dependency` - Remove branch dependency
- `twig_add_root_branch` - Add root branch
- `twig_remove_root_branch` - Remove root branch
- `twig_list_root_branches` - List root branches
- `twig_tidy_clean` - Clean up branches
- `twig_tidy_prune` - Prune stale references
- `twig_github_create_pr` - Create GitHub PR

## Development

```bash
npm run dev
```

## Requirements

- Node.js 18+
- twig CLI tool installed and available in PATH
- Git repository with twig configuration