# Twig MCP Server - VS Code Installation Guide

## ✅ Installation Complete!

The Twig MCP server is now installed and configured for VS Code. Here's what was done:

### 1. ✅ Twig Installed Globally
- Location: `C:\Users\bkrup\.cargo\bin\twig.exe`
- Command `twig` is now available in your PATH

### 2. ✅ VS Code Settings Configured
- File: `.vscode/settings.json`
- MCP server configured to run automatically with GitHub Copilot

## 🚀 Next Steps

### Step 1: Restart VS Code
Close and reopen VS Code (or reload the window) to activate the MCP server.

**How to reload:**
1. Press `Ctrl+Shift+P`
2. Type "Developer: Reload Window"
3. Press Enter

### Step 2: Verify Installation

Once VS Code restarts:

1. **Open GitHub Copilot Chat** (click the chat icon in the sidebar or press `Ctrl+Alt+I`)

2. **Check MCP Status** (optional):
   - Press `Ctrl+Shift+P`
   - Type "Output"
   - Select "View: Toggle Output"
   - From the dropdown, select "GitHub Copilot Chat"
   - Look for messages about "twig" MCP server

3. **Test with a Question**:
   Ask Copilot:
   ```
   What branches do I have in this repository?
   ```
   
   Or try:
   ```
   Show me the twig branch tree
   ```

## 📝 What You Can Ask

Here are some example questions you can ask GitHub Copilot:

### Branch Information
- "What branches do I have?"
- "List all my branches with their Jira issues"
- "Show me branches with GitHub PRs"
- "What's the status of the feature/xyz branch?"

### Repository Management
- "Which repositories are registered with twig?"
- "Show me all worktrees"
- "What's in my twig registry?"

### Dependency Tracking
- "Show me the branch dependency tree"
- "What branches depend on main?"

## 🔧 Configuration Details

Your current configuration in `.vscode/settings.json`:

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

### Enable Debug Logging

If you need more detailed logs for troubleshooting, change `info` to `debug`:

```json
"env": {
  "RUST_LOG": "debug,twig_mcp=trace"
}
```

## 🐛 Troubleshooting

### Server Not Starting?

1. **Check Output Panel:**
   - View → Output
   - Select "GitHub Copilot Chat" from dropdown
   - Look for error messages

2. **Verify twig command:**
   ```powershell
   twig --version
   ```

3. **Test MCP server manually:**
   ```powershell
   echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | twig mcp-server
   ```
   Should return a JSON response with server info.

### Copilot Not Responding?

1. Reload VS Code window
2. Check that GitHub Copilot extension is active
3. Try disabling/re-enabling the Copilot extension

### Need More Help?

- Full documentation: `twig-mcp/README.md`
- Check logs in VS Code Output panel
- File an issue on GitHub

## 🎉 You're All Set!

The Twig MCP server is now integrated with GitHub Copilot in VS Code. You can now interact with your Git repository using natural language through AI assistance!
