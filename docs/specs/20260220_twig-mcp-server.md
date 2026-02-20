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
- **Structured responses.** Every tool returns a JSON-serialized `ToolResponse<T>` envelope — either `{ "status": "ok", "data": ... }` or `{ "status": "error", "error": { "code", "message", "hint?" } }`. This lets MCP clients reliably parse and reuse data instead of ad-hoc text parsing.
- **Consistent error shape.** All errors use the same `ToolError` struct with a machine-readable `code`, a human-readable `message`, and an optional `hint`. See the _Error codes_ table for the standardized code vocabulary.
- **Graceful degradation.** If Jira or GitHub credentials are missing, or the server is started outside a repo, all tools are still listed. Each tool handles missing context individually — returning structured errors with actionable hints — rather than crashing. See the _Degraded Context Behavior_ section for the full context dependency matrix.

## Context: MCP Protocol Primitives

MCP servers expose three kinds of primitives to clients:

1. **Tools** — Callable functions with typed input schemas and structured output. The client (AI) decides when to call them. This is the primary primitive for twig-mcp.
2. **Resources** — Data objects the client can read/subscribe to (files, database records, etc.). Useful for semi-static context like "the branch dependency tree."
3. **Prompts** — Reusable instruction templates. Lower priority for v1 but could be useful for "summarize my stack status."

For v1, **tools are the main surface area**. Resources and prompts are stretch goals.

## Target Capabilities

### 1. Tools (P0 — core surface area)

Each tool receives JSON parameters and returns structured JSON content. The AI assistant calls these tools as needed during conversation.

#### Local state tools (no network, fast)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_current_branch` | Current git branch name and associated metadata | _none_ | `BranchMetadataResponse` |
| `get_branch_metadata` | Metadata for a specific branch | `branch: string` | `BranchMetadataResponse` |
| `get_branch_tree` | Dependency tree for the current repo | `branch?: string` (optional root) | `BranchTreeResponse` |
| `get_branch_stack` | Ancestor chain from a branch to its root | `branch?: string` (defaults to current) | `BranchStackResponse` |
| `list_branches` | All twig-tracked branches in current repo | _none_ | `ListBranchesResponse` |
| `list_repositories` | All twig-registered repositories | _none_ | `ListRepositoriesResponse` |
| `get_worktrees` | Active worktrees for current repo | _none_ | `ListWorktreesResponse` |

#### GitHub tools (network, requires credentials)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_pull_request` | Full PR details | `pr_number?: u32` (defaults to current branch's PR) | `PullRequestResponse` |
| `get_pr_status` | PR with reviews and CI checks | `pr_number?: u32` (defaults to current branch's PR) | `PrStatusResponse` |
| `list_pull_requests` | Open PRs for current repo | `state?: string` (default "open") | `ListPullRequestsResponse` |

#### Jira tools (network, requires credentials)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_jira_issue` | Full issue details | `issue_key?: string` (defaults to current branch's issue) | `JiraIssueResponse` |
| `list_jira_issues` | Issues for a project | `project: string`, `status?: string`, `assignee?: string` | `ListJiraIssuesResponse` |

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
│       ├── types.rs          # ToolResponse<T>, ToolError, all response/error structs
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

# MCP SDK (official Rust SDK: github.com/modelcontextprotocol/rust-sdk)
rmcp = { version = "0.15", features = ["server", "transport-io"] }
schemars = "0.8"   # JSON Schema generation for tool parameters

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

Using `rmcp` macros, each tool is a method on the server struct. The `#[tool(tool_box)]` macro is applied to both the impl block (defining tools) and the `ServerHandler` impl (wiring up protocol handling):

```rust
use rmcp::{ServerHandler, ServiceExt, model::*, schemars, tool, transport::stdio};

/// Parameter types use schemars for automatic JSON Schema generation.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BranchMetadataParams {
    /// The branch name to look up
    #[schemars(description = "Branch name (defaults to current branch if omitted)")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TwigMcpServer {
    context: Arc<ServerContext>,
}

/// All tools are annotated with read_only_hint = true since this is a
/// read-only server. The idempotent_hint tells clients that repeated
/// calls with the same params produce the same result.
#[tool(tool_box)]
impl TwigMcpServer {
    pub fn new(context: ServerContext) -> Self {
        Self { context: Arc::new(context) }
    }

    #[tool(
        description = "Get the current git branch and its linked Jira issue and GitHub PR",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn get_current_branch(&self) -> String {
        let repo_path = self.context.require_repo()?;

        let branch = twig_core::git::current_branch();
        let Ok(Some(branch_name)) = branch else {
            return serde_json::to_string(&ToolResponse::<BranchMetadataResponse>::Error {
                error: ToolError {
                    code: "not_found".into(),
                    message: "Not on any branch (detached HEAD state)".into(),
                    hint: None,
                },
            })?;
        };

        // Load fresh state on every call — degrade gracefully if missing
        let (jira_issue, pr_number, parent_branch) =
            match self.context.load_repo_state() {
                Ok(state) => {
                    let meta = state.branches.get(&branch_name);
                    (
                        meta.and_then(|m| m.jira_issue.clone()),
                        meta.and_then(|m| m.github_pr),
                        meta.and_then(|m| m.parent.clone()),
                    )
                }
                Err(_) => (None, None, None),
            };

        serde_json::to_string(&ToolResponse::Ok {
            data: BranchMetadataResponse {
                branch: branch_name,
                jira_issue,
                pr_number,
                parent_branch,
                created_at: None,
            },
        })?
    }

    #[tool(
        description = "Get metadata for a specific branch (Jira issue, PR number, creation date)",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn get_branch_metadata(
        &self,
        #[tool(aggr)] params: BranchMetadataParams,
    ) -> String {
        // ... resolve branch name, load state, return metadata
        todo!()
    }

    // ... more tools follow the same pattern
}

#[tool(tool_box)]
impl ServerHandler for TwigMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Twig MCP server. Provides read-only access to branch metadata, \
                 Jira issues, and GitHub PRs for the current repository.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
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

## Structured Response Types

All tools return JSON-serialized response structs rather than ad-hoc formatted strings. This allows MCP clients to reliably parse and reuse data without text parsing. Responses are serialized to JSON and returned as MCP `text` content.

### Response structs

```rust
/// Standard wrapper for all tool responses. Every tool returns either
/// a typed `data` payload or a structured error.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "status")]
pub enum ToolResponse<T: serde::Serialize> {
    #[serde(rename = "ok")]
    Ok { data: T },
    #[serde(rename = "error")]
    Error { error: ToolError },
}

/// Consistent error shape returned by all tools.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ToolError {
    /// Machine-readable error code (e.g., "no_repo", "credentials_missing",
    /// "not_found", "network_error", "invalid_params").
    pub code: String,
    /// Human-readable error description.
    pub message: String,
    /// Optional actionable suggestion for resolving the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Response for `get_current_branch` and `get_branch_metadata`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct BranchMetadataResponse {
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_issue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Response for `get_branch_tree`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct BranchTreeResponse {
    pub root: String,
    pub tree_text: String,
    pub branches: Vec<BranchTreeNode>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct BranchTreeNode {
    pub branch: String,
    pub children: Vec<BranchTreeNode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_issue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u32>,
}

/// Response for `get_branch_stack`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct BranchStackResponse {
    /// Ordered from the queried branch (index 0) up to the root.
    pub stack: Vec<BranchMetadataResponse>,
}

/// Response for `list_branches`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ListBranchesResponse {
    pub branches: Vec<BranchMetadataResponse>,
}

/// Response for `list_repositories`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ListRepositoriesResponse {
    pub repositories: Vec<RepositoryInfo>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct RepositoryInfo {
    pub name: String,
    pub path: String,
}

/// Response for `get_worktrees`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ListWorktreesResponse {
    pub worktrees: Vec<WorktreeInfo>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: String,
    pub branch: String,
}

/// Response for `get_pull_request`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PullRequestResponse {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: String,
    pub base: String,
    pub head: String,
    pub draft: bool,
    pub mergeable: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
}

/// Response for `get_pr_status`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PrStatusResponse {
    pub pull_request: PullRequestResponse,
    pub reviews: Vec<ReviewInfo>,
    pub checks: Vec<CheckRunInfo>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ReviewInfo {
    pub author: String,
    pub state: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct CheckRunInfo {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// Response for `list_pull_requests`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ListPullRequestsResponse {
    pub pull_requests: Vec<PullRequestResponse>,
}

/// Response for `get_jira_issue`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct JiraIssueResponse {
    pub key: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

/// Response for `list_jira_issues`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ListJiraIssuesResponse {
    pub issues: Vec<JiraIssueResponse>,
}
```

### Error codes

All tools use the `ToolError` struct with the following standardized error codes:

| Code | Meaning | Typical hint |
|------|---------|-------------|
| `no_repo` | twig-mcp was started outside a git repository, or the repo was deleted | "Run twig-mcp from within a git repository, or pass `--repo <path>`." |
| `no_twig_state` | Repository exists but has no `.twig/state.json` | "Run `twig init` in this repository first." |
| `credentials_missing` | `~/.netrc` does not contain credentials for the required service | "Add credentials for {host} to `~/.netrc`. See `twig auth --help`." |
| `not_found` | The requested branch, PR, or issue does not exist | (varies) |
| `invalid_params` | The caller provided invalid or missing parameters | (varies, describes the specific validation failure) |
| `network_error` | A request to GitHub or Jira failed due to connectivity or HTTP error | "Check your network connection. HTTP status: {status}." |
| `rate_limited` | GitHub or Jira API rate limit exceeded | "Rate limited by {service}. Retry after {retry_after}." |
| `internal` | Unexpected error within twig-mcp | "This is a bug. Please report it." |

When a tool encounters an error, it returns `ToolResponse::Error` with the `is_error: true` flag set on the MCP `CallToolResult`, so that MCP clients can distinguish success from failure without inspecting the payload.

## Degraded Context Behavior

`twig-mcp` may be started in environments where some context is unavailable — no git repo, no twig state, missing credentials, etc. Rather than failing to start, the server always starts and lists all tools. Each tool handles missing context individually according to the matrix below.

### Context dependency matrix

| Tool | Requires repo? | Requires twig state? | Requires GitHub creds? | Requires Jira creds? |
|------|:---:|:---:|:---:|:---:|
| `get_current_branch` | Yes | Partial (degrades) | No | No |
| `get_branch_metadata` | Yes | Yes | No | No |
| `get_branch_tree` | Yes | Yes | No | No |
| `get_branch_stack` | Yes | Yes | No | No |
| `list_branches` | Yes | Yes | No | No |
| `list_repositories` | No | No | No | No |
| `get_worktrees` | Yes | Yes | No | No |
| `get_pull_request` | Partial (needs repo for default PR) | Partial (needs state for default PR) | Yes | No |
| `get_pr_status` | Partial (needs repo for default PR) | Partial (needs state for default PR) | Yes | No |
| `list_pull_requests` | Yes (needs remote URL) | No | Yes | No |
| `get_jira_issue` | Partial (needs state for default issue) | Partial (needs state for default issue) | No | Yes |
| `list_jira_issues` | No | No | No | Yes |

**Legend:**
- **Yes** — tool returns `ToolError` with the appropriate code if this context is missing.
- **Partial (degrades)** — tool still works but with reduced information. For example, `get_current_branch` without twig state returns just the branch name without Jira/PR associations; `get_pull_request` with explicit `pr_number` works without a repo context.
- **No** — tool does not need this context at all.

### Behavior by context scenario

**No git repository (started outside a repo, no `--repo` flag):**
- `list_repositories` and `list_jira_issues` (with explicit `project`) work normally.
- `get_pull_request` and `get_pr_status` work if `pr_number` is provided explicitly (repo owner/name inferred from explicit params or returns error).
- All other tools return `ToolError { code: "no_repo", ... }`.

**Repository exists but no `.twig/state.json`:**
- `get_current_branch` returns the branch name only (no Jira/PR metadata).
- `list_repositories`, `get_worktrees` work normally.
- GitHub tools with explicit `pr_number` work normally.
- Tools that need branch metadata return `ToolError { code: "no_twig_state", ... }`.

**Missing GitHub credentials:**
- All local and Jira tools work normally.
- GitHub tools return `ToolError { code: "credentials_missing", ... }` with a hint to configure `~/.netrc`.

**Missing Jira credentials:**
- All local and GitHub tools work normally.
- Jira tools return `ToolError { code: "credentials_missing", ... }` with a hint to configure `~/.netrc`.

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
| P0 | Scaffold `twig-mcp` crate | Cargo.toml with correct dependencies, added to workspace members, `main.rs` compiles and starts an empty MCP server over stdio. Includes `types.rs` with `ToolResponse<T>`, `ToolError`, and all response structs. | Use rmcp with `server` + `transport-io` features. Verify `cargo build -p twig-mcp` succeeds. | |
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
