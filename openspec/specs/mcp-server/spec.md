# MCP Server

## Purpose

Expose twig capabilities to AI assistants (Claude Code, etc.) via the Model Context Protocol. Provides 12 read-only
tools: 7 local state tools, 3 GitHub tools, and 2 Jira tools. Runs as a standalone binary (`twig-mcp`) communicating
over stdio transport.

**Binary:** `twig-mcp` **Crates:** `twig-mcp` (server, context, tools, types) **Dependencies:** `rmcp` v0.15,
`twig-core`, `twig-gh`, `twig-jira`

## Requirements

<!-- Requirements will be seeded from existing behavior -->
