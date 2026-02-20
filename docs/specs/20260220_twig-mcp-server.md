# Twig MCP Server

## Purpose

- Expose twig's branch metadata, Jira issue details, and GitHub PR status as a read-only MCP server so that AI assistants (Claude Code, Claude Desktop, etc.) can reason about the developer's current working context without manual copy-pasting.
- Ship as a standalone `twig-mcp` binary that reuses existing `twig-core`, `twig-gh`, and `twig-jira` crates. Users install it and register it as an MCP server — no changes to the main `twig` binary required.
- Read-only in v1. No mutations (no creating branches, transitioning Jira issues, or merging PRs).

### Non-goals

- Replacing `twig` CLI commands. The MCP server surfaces _information_, not workflows.
- Remote/multi-user deployment. This is a local stdio server, same trust model as the existing CLI.
- Supporting MCP clients other than Claude (though the protocol is standard, we won't test against other clients in v1).

## Guiding Constraints

- **Isolation.** `twig-mcp` is a new workspace crate that produces its own binary. It depends on `twig-core`, `twig-gh`, and `twig-jira` as library crates but does not depend on `twig-cli`. Users can install it independently.
- **Read-only.** Every tool is a query. No side effects on git state, Jira, or GitHub. This keeps the trust boundary simple — an AI assistant cannot break anything by calling twig-mcp.
- **Credential reuse.** Auth uses the same `~/.netrc` credentials that `twig` already relies on. No new auth configuration.
- **Stdio transport.** The server communicates over stdin/stdout using JSON-RPC 2.0 (MCP standard). This is the default transport for local MCP servers in Claude Code and Claude Desktop.
- **Workspace lints.** `twig-mcp` inherits the workspace clippy lints (no unwrap, no panic, no print_stdout/stderr, etc.). All user-facing output goes through the MCP protocol, never raw stdout.
- **Graceful degradation.** If Jira or GitHub credentials are missing, those tools should still be listed but return clear error messages when called. The server should not crash on missing config.

## Context: MCP Protocol Primitives

MCP servers expose three kinds of primitives to clients:

1. **Tools** — Callable functions with typed input schemas and structured output. The client (AI) decides when to call them. This is the primary primitive for twig-mcp.
2. **Resources** — Data objects the client can read/subscribe to (files, database records, etc.). Useful for semi-static context like "the branch dependency tree."
3. **Prompts** — Reusable instruction templates. Lower priority for v1 but could be useful for "summarize my stack status."

For v1, **tools are the main surface area**. Resources and prompts are stretch goals.

## Target Capabilities

### 1. Tools (P0 — core surface area)

Each tool receives JSON parameters and returns structured text content. The AI assistant calls these tools as needed during conversation.

#### Local state tools (no network, fast)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_current_branch` | Current git branch name and associated metadata | _none_ | Branch name, linked Jira issue key (if any), linked PR number (if any) |
| `get_branch_metadata` | Metadata for a specific branch | `branch: string` | Branch name, Jira issue, PR number, creation date |
| `get_branch_tree` | Dependency tree for the current repo | `branch?: string` (optional root) | Text rendering of the branch dependency graph (same format as `twig tree`) |
| `get_branch_stack` | Ancestor chain from a branch to its root | `branch?: string` (defaults to current) | Ordered list: branch → parent → ... → root, with metadata per branch |
| `list_branches` | All twig-tracked branches in current repo | _none_ | List of branch names with their Jira/PR associations |
| `list_repositories` | All twig-registered repositories | _none_ | List of repo names and paths from the global registry |
| `get_worktrees` | Active worktrees for current repo | _none_ | List of worktree name, path, branch |

#### GitHub tools (network, requires credentials)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_pull_request` | Full PR details | `pr_number?: u32` (defaults to current branch's PR) | Title, state, author, base/head, draft status, mergeable, created/updated timestamps |
| `get_pr_status` | PR with reviews and CI checks | `pr_number?: u32` (defaults to current branch's PR) | PR details + review states + check run results |
| `list_pull_requests` | Open PRs for current repo | `state?: string` (default "open") | List of PRs with number, title, author, state |

#### Jira tools (network, requires credentials)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_jira_issue` | Full issue details | `issue_key?: string` (defaults to current branch's issue) | Key, summary, description, status, assignee |
| `list_jira_issues` | Issues for a project | `project: string`, `status?: string`, `assignee?: string` | List of issues with key, summary, status |

### 2. Resources (P2 — stretch goal)

| Resource URI | Description |
|--------------|-------------|
| `twig://repo/tree` | Branch dependency tree for the current repository (subscribable) |
| `twig://repo/branches` | All tracked branches with metadata |
| `twig://branch/{name}/context` | Combined view: branch metadata + Jira issue + PR status |

### 3. Prompts (P3 — future)

| Prompt | Description |
|--------|-------------|
| `stack-status` | "Summarize the current state of my PR stack, including review status and CI results." |
| `branch-context` | "What am I working on? Describe the current branch's Jira issue and PR." |

## Architecture

### Workspace layout

```
twig/
├── twig-mcp/                 # New crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # Entry point: init tracing, detect repo, start MCP server
│       ├── server.rs         # TwigMcpServer struct, ServerHandler impl
│       ├── tools/
│       │   ├── mod.rs
│       │   ├── local.rs      # get_current_branch, get_branch_tree, etc.
│       │   ├── github.rs     # get_pull_request, get_pr_status, etc.
│       │   └── jira.rs       # get_jira_issue, list_jira_issues
│       └── context.rs        # ServerContext: holds ConfigDirs, RepoState, clients
├── twig-core/                # Existing (no changes expected)
├── twig-gh/                  # Existing (no changes expected)
├── twig-jira/                # Existing (no changes expected)
└── Cargo.toml                # Add twig-mcp to workspace members
```

### Server lifecycle

```
1. User configures MCP client (Claude Code / Desktop) to run `twig-mcp`
2. Client spawns `twig-mcp` as a child process
3. twig-mcp initializes:
   a. Detect current git repository (same as twig CLI)
   b. Load ConfigDirs (XDG paths)
   c. Load RepoState from .twig/state.json
   d. Optionally create GitHub/Jira clients from ~/.netrc (lazy, on first use)
4. MCP handshake over stdin/stdout (JSON-RPC 2.0)
5. Client discovers tools via tools/list
6. Client calls tools as needed during conversation
7. Client terminates process when session ends
```

### ServerContext struct

```rust
/// Shared context available to all tool handlers.
struct ServerContext {
    config_dirs: ConfigDirs,
    repo_path: Option<PathBuf>,

    // Lazily initialized on first network call
    github_client: OnceCell<Option<GitHubClient>>,
    jira_client: OnceCell<Option<JiraClient>>,
}
```

Key design decisions:
- **Repo detection at startup.** The server detects the git repository once and serves tools in that context. If `twig-mcp` is started outside a git repo, local tools return errors but the server still runs (GitHub/Jira tools with explicit parameters still work).
- **Lazy client init.** GitHub and Jira clients are created on first use. If credentials are missing, the tool returns a descriptive error rather than crashing.
- **State reload.** RepoState is reloaded from disk on each tool call (it's a small JSON file). This ensures the MCP server always reflects the latest `twig` state without needing file watchers.

### Dependencies (Cargo.toml sketch)

```toml
[package]
name = "twig-mcp"
version.workspace = true
edition.workspace = true

[[bin]]
name = "twig-mcp"
path = "src/main.rs"

[dependencies]
twig-core = { path = "../twig-core" }
twig-gh = { path = "../twig-gh" }
twig-jira = { path = "../twig-jira" }

# MCP SDK
rmcp = { version = "0.15", features = ["server", "transport-io"] }

# Async runtime (rmcp requires tokio)
tokio = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Error handling
anyhow = { workspace = true }

# Logging (MCP server must not print to stdout; tracing goes to stderr)
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

### Tool implementation pattern

Using `rmcp` macros, each tool is a method on the server struct:

```rust
use rmcp::{ServerHandler, tool, tool_router, tool_handler, model::*};

#[derive(Clone)]
pub struct TwigMcpServer {
    context: Arc<ServerContext>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TwigMcpServer {
    pub fn new(context: ServerContext) -> Self {
        let context = Arc::new(context);
        Self {
            context,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get the current git branch and its linked Jira issue and GitHub PR")]
    async fn get_current_branch(&self) -> Result<CallToolResult, McpError> {
        let branch = twig_core::git::current_branch()
            .map_err(|e| McpError::internal_error(e.to_string()))?;

        let Some(branch_name) = branch else {
            return Ok(CallToolResult::success(vec![
                Content::text("Not on any branch (detached HEAD state)")
            ]));
        };

        // Load fresh state
        let state = self.context.load_repo_state()?;
        let metadata = state.branches.get(&branch_name);

        // Format response
        let mut lines = vec![format!("Branch: {branch_name}")];
        if let Some(meta) = metadata {
            if let Some(ref issue) = meta.jira_issue {
                lines.push(format!("Jira: {issue}"));
            }
            if let Some(pr) = meta.github_pr {
                lines.push(format!("PR: #{pr}"));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
    }

    // ... more tools
}

#[tool_handler]
impl ServerHandler for TwigMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new("twig-mcp", env!("CARGO_PKG_VERSION"))
            .with_instructions("Twig MCP server. Provides read-only access to branch metadata, Jira issues, and GitHub PRs for the current repository.")
            .enable_tools()
    }
}
```

### main.rs sketch

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing to stderr (stdout is reserved for MCP protocol)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("twig_mcp=info")
        .init();

    let config_dirs = ConfigDirs::new()?;
    let repo_path = twig_core::git::detect_repository();

    let context = ServerContext::new(config_dirs, repo_path);
    let server = TwigMcpServer::new(context);

    // Start MCP server on stdio
    let transport = rmcp::transport::stdio::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
```

## User Configuration

### Claude Code

```bash
# One-time setup
claude mcp add twig-mcp --scope user -- twig-mcp

# Or via ~/.claude.json
```

```json
{
  "mcpServers": {
    "twig-mcp": {
      "type": "stdio",
      "command": "twig-mcp"
    }
  }
}
```

### Claude Desktop

```json
{
  "mcpServers": {
    "twig-mcp": {
      "command": "twig-mcp",
      "args": []
    }
  }
}
```

### Project-level (.mcp.json)

For teams that want twig-mcp available automatically:

```json
{
  "mcpServers": {
    "twig-mcp": {
      "type": "stdio",
      "command": "twig-mcp"
    }
  }
}
```

## Installation

```bash
# From the twig workspace
cargo install --path twig-mcp

# Or via cargo install once published
cargo install twig-mcp
```

The binary name `twig-mcp` follows the existing plugin naming convention (`twig-<name>`), so it could also be discovered as a twig plugin in the future. However, unlike plugins, it is not invoked by the `twig` CLI — it is invoked directly by the MCP client.

## Subagent Execution Plan

### Task Backlog

| Priority | Task | Definition of Done | Notes | Status |
| -------- | ---- | ------------------ | ----- | ------ |
| P0 | Scaffold `twig-mcp` crate | Cargo.toml with correct dependencies, added to workspace members, `main.rs` compiles and starts an empty MCP server over stdio | Use rmcp with `server` + `transport-io` features. Verify `cargo build -p twig-mcp` succeeds. | |
| P0 | Implement `get_current_branch` tool | Returns branch name + linked Jira/PR from RepoState | First tool — validates the full rmcp macro pipeline works end-to-end. | |
| P0 | Implement `get_branch_metadata` tool | Given a branch name, returns its metadata from RepoState | | |
| P0 | Implement `get_branch_tree` tool | Returns text rendering of the dependency tree | Reuse `twig_core::git::graph` and `renderer` modules. Need to check if renderer can output to a String rather than stdout. | |
| P0 | Implement `get_branch_stack` tool | Returns ancestor chain from branch to root | Walk `dependency_parents_index` iteratively. | |
| P0 | Implement `list_branches` tool | Returns all tracked branches with associations | Iterate `RepoState.branches`. | |
| P1 | Implement `get_pull_request` tool | Returns PR details; defaults to current branch's PR | Uses `twig_gh::endpoints::pulls::get_pull_request`. Lazy client init. | |
| P1 | Implement `get_pr_status` tool | Returns PR + reviews + checks | Composes `get_pull_request` + reviews + check runs endpoints. | |
| P1 | Implement `list_pull_requests` tool | Returns open PRs for current repo | Uses `twig_gh::endpoints::pulls::list_pull_requests`. | |
| P1 | Implement `get_jira_issue` tool | Returns issue details; defaults to current branch's issue | Uses `twig_jira::endpoints::issues::get_issue`. Lazy client init. | |
| P1 | Implement `list_jira_issues` tool | Returns issues for a project with optional filters | Uses `twig_jira::endpoints::issues::list_issues`. | |
| P1 | Implement `list_repositories` tool | Returns all repos from global registry | Uses `Registry::load()`. | |
| P1 | Implement `get_worktrees` tool | Returns active worktrees for current repo | Reads from `RepoState.worktrees`. | |
| P2 | Add integration tests | Test tool responses against mock state and wiremock for network calls | Use `twig-test-utils` fixtures. Can test the MCP layer by calling tool handlers directly. | |
| P2 | Add `twig-mcp` to `make build` / `make release` targets | Makefile builds twig-mcp alongside twig | | |
| P2 | Resources: `twig://repo/tree` | Expose branch tree as an MCP resource | Requires `enable_resources()` in ServerInfo. | |
| P3 | Prompts: `stack-status`, `branch-context` | Reusable prompt templates | Requires `enable_prompts()` in ServerInfo. | |

### Risks & Mitigations

- **Risk:** `twig-core` renderer writes directly to stdout/console with color codes. **Mitigation:** The tree renderer already builds intermediate data structures (`BranchNode`, `BranchNodeMetadata`). We can either (a) add a `render_to_string` variant that strips ANSI, or (b) build a simpler text renderer in `twig-mcp` that walks the tree data structures directly. Option (b) is preferred to avoid coupling.

- **Risk:** `rmcp` crate is relatively new and may have breaking changes. **Mitigation:** Pin to a specific minor version. The server is simple enough (tools only, stdio transport) that the API surface we use is small and stable.

- **Risk:** GitHub/Jira clients in `twig-gh`/`twig-jira` use `reqwest::blocking` in some paths, which conflicts with an async tokio runtime. **Mitigation:** Audit the client constructors. The `create_github_runtime_and_client` functions create their own tokio runtime, which won't work inside an existing runtime. Use `create_github_client_from_netrc` instead and call async endpoints directly since we're already in a tokio context.

- **Risk:** MCP server process lifetime vs. repo context. The server is started once but the user may switch branches during a session. **Mitigation:** Reload `RepoState` and re-detect `current_branch()` on every tool call. The git operations are cheap (read HEAD ref).

### Open Questions

- Should `twig-mcp` accept a `--repo <path>` argument to explicitly set the repository context, or always auto-detect from cwd?
- Should network tools (GitHub/Jira) have configurable timeouts exposed as CLI flags?
- Should `twig mcp` be added as a subcommand of the main `twig` binary that just exec's `twig-mcp`? This would make discoverability easier but adds a dependency.

## Status Tracking (to be updated by subagent)

- **Current focus:** _Design phase — spec authoring._
- **Latest completed task:** _N/A_
- **Next up:** _P0: Scaffold `twig-mcp` crate._

## Lessons Learned (ongoing)

- _To be updated during implementation._
