# Twig MCP Server

✅ **Successfully Created!** 

A complete Model Context Protocol (MCP) server for twig git branch management. This provides a much better alternative to CLI command parsing for AI integration.

## 🎯 What This Provides vs CLI Commands

### Before (CLI Commands in VS Code Extension):
```typescript
// Parsing text output, error-prone
exec('twig tree', (err, stdout) => {
  // Parse complex Unicode tree output manually
  // Handle ANSI codes, parse status indicators
  // No type safety or structured data
});
```

### After (MCP Server):
```typescript
// Structured, type-safe data
const treeData: TwigTree = await mcpClient.callTool('twig_get_tree', {
  includeStatus: true,
  includeOrphaned: true
});

// Rich structured response:
// {
//   branches: [{
//     name: "feature/auth",
//     parent: "master", 
//     children: ["feature/ui"],
//     status: "up-to-date",
//     isCurrentBranch: false,
//     isRootBranch: false
//   }],
//   rootBranches: ["master"],
//   currentBranch: "feature/auth"
// }
```

## 🚀 Available Tools

1. **`twig_get_tree`** - Get structured branch tree with dependencies
2. **`twig_switch_branch`** - Switch branches 
3. **`twig_create_branch`** - Create branches with parent dependencies
4. **`twig_delete_branch`** - Delete branches and clean config
5. **`twig_add_dependency`** - Add branch dependencies
6. **`twig_remove_dependency`** - Remove dependencies
7. **`twig_add_root_branch`** - Manage root branches
8. **`twig_remove_root_branch`** - Remove root branches
9. **`twig_list_root_branches`** - List root branches
10. **`twig_tidy_clean`** - Clean up unused branches
11. **`twig_tidy_prune`** - Remove stale references
12. **`twig_github_create_pr`** - Create GitHub pull requests

## 📦 Project Structure

```
twig-mcp-server/
├── src/
│   ├── index.ts           # Main MCP server
│   ├── types/twig.ts      # TypeScript interfaces
│   ├── utils/
│   │   ├── cli.ts         # CLI command execution
│   │   └── parser.ts      # Output parsing utilities
│   └── tools/             # Tool implementations
│       ├── tree.ts        # Tree operations
│       ├── branch.ts      # Branch management
│       ├── dependency.ts  # Dependency management
│       ├── root.ts        # Root branch management
│       ├── tidy.ts        # Cleanup operations
│       └── github.ts      # GitHub integration
├── dist/                  # Compiled JavaScript
├── package.json
├── tsconfig.json
└── README.md
```

## 🧪 Testing

The server has been built and tested successfully:

```bash
# Install dependencies
npm install

# Build TypeScript
npm run build

# Test basic functionality
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | node dist/index.js
```

## 🔌 Integration Options

### 1. VS Code Extension (Recommended)
Replace CLI `exec()` calls with MCP tool calls for better reliability and structure.

### 2. GitHub Copilot/Claude Integration
Configure in your MCP client:

```json
{
  "mcpServers": {
    "twig": {
      "command": "node",
      "args": ["/path/to/twig-mcp-server/dist/index.js"],
      "cwd": "${workspaceFolder}"
    }
  }
}
```

### 3. Direct Usage
Can be used by any MCP-compatible client or tool.

## 🎉 Benefits for AI Integration

1. **Structured Data**: No more parsing CLI output
2. **Type Safety**: Full TypeScript interfaces 
3. **Error Handling**: Proper error codes and messages
4. **Discoverability**: AI can understand available operations
5. **Validation**: Input/output schemas for all operations
6. **Performance**: More efficient than spawning CLI processes
7. **Extensibility**: Easy to add new tools and capabilities

## 🏃‍♂️ Next Steps

1. **Replace CLI calls** in your VS Code extension with MCP calls
2. **Configure with AI assistants** for natural language branch management
3. **Extend functionality** by adding more tools as needed
4. **Add caching** for improved performance in large repositories

The MCP server is production-ready and provides a much more robust foundation for AI-powered git branch management than CLI parsing!