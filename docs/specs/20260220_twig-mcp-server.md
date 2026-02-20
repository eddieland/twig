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
- **Graceful degradation.** If Jira or GitHub credentials are missing, those tools are still listed but return structured `ToolError` responses (with `code`, `message`, `hint`) when called. The server never crashes on missing config. See the [Context Availability Matrix](#context-availability-matrix) for per-tool behavior in each degraded state.
- **Structured responses.** Every tool returns a JSON object as its MCP text content — never ad-hoc formatted text. Success responses use typed structs (e.g., `CurrentBranchResponse`); error responses use the standard `ToolError` shape. This lets MCP clients parse and reuse data programmatically.

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
| `get_current_branch` | Current git branch name and associated metadata | _none_ | `CurrentBranchResponse` — branch, jira_issue, pr_number, created_at |
| `get_branch_metadata` | Metadata for a specific branch | `branch: string` | `BranchMetadataResponse` — branch, jira_issue, pr_number, created_at, parents, children |
| `get_branch_tree` | Dependency tree for the current repo | `branch?: string` (optional root) | `BranchTreeResponse` — text rendering + structured node list |
| `get_branch_stack` | Ancestor chain from a branch to its root | `branch?: string` (defaults to current) | `BranchStackResponse` — ordered stack entries from branch to root |
| `list_branches` | All twig-tracked branches in current repo | _none_ | `ListBranchesResponse` — list of `BranchSummary` |
| `list_repositories` | All twig-registered repositories | _none_ | `ListRepositoriesResponse` — list of `RepositorySummary` |
| `get_worktrees` | Active worktrees for current repo | _none_ | `ListWorktreesResponse` — list of `WorktreeSummary` |

#### GitHub tools (network, requires credentials)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_pull_request` | Full PR details | `pr_number?: u32` (defaults to current branch's PR) | `PullRequestResponse` — number, title, url, state, author, base/head, draft, mergeable, timestamps |
| `get_pr_status` | PR with reviews and CI checks | `pr_number?: u32` (defaults to current branch's PR) | `PullRequestStatusResponse` — pr + reviews + check_runs |
| `list_pull_requests` | Open PRs for current repo | `state?: string` (default "open") | `ListPullRequestsResponse` — list of `PullRequestSummary` |

#### Jira tools (network, requires credentials)

| Tool | Description | Parameters | Returns |
|------|-------------|------------|---------|
| `get_jira_issue` | Full issue details | `issue_key?: string` (defaults to current branch's issue) | `JiraIssueResponse` — key, summary, description, status, assignee, updated |
| `list_jira_issues` | Issues for a project | `project: string`, `status?: string`, `assignee?: string` | `ListJiraIssuesResponse` — list of `JiraIssueSummary` |

### Structured Response Types

All tools return JSON-serialized structured responses rather than ad-hoc text. This allows MCP clients to reliably parse fields (branch names, PR numbers, issue keys, etc.) without text scraping. Each tool serializes its response type to JSON and returns it as a single MCP text content block.

The response types are defined as Rust structs with `Serialize` and `JsonSchema`. They mirror the existing domain types from `twig-core`, `twig-gh`, and `twig-jira` but are flattened for MCP consumption — no nested opaque types, no internal IDs.

#### Local state responses

```rust
/// get_current_branch
#[derive(Serialize, JsonSchema)]
pub struct CurrentBranchResponse {
    pub branch: String,
    pub jira_issue: Option<String>,
    pub pr_number: Option<u32>,
    pub created_at: Option<String>,
}

/// get_branch_metadata
#[derive(Serialize, JsonSchema)]
pub struct BranchMetadataResponse {
    pub branch: String,
    pub jira_issue: Option<String>,
    pub pr_number: Option<u32>,
    pub created_at: String,
    pub parents: Vec<String>,
    pub children: Vec<String>,
}

/// get_branch_tree — retains a text rendering for human readability,
/// but also includes the structured node list for programmatic use.
#[derive(Serialize, JsonSchema)]
pub struct BranchTreeResponse {
    pub text: String,
    pub nodes: Vec<BranchTreeNode>,
}

#[derive(Serialize, JsonSchema)]
pub struct BranchTreeNode {
    pub branch: String,
    pub jira_issue: Option<String>,
    pub pr_number: Option<u32>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}

/// get_branch_stack
#[derive(Serialize, JsonSchema)]
pub struct BranchStackResponse {
    /// Ordered from the requested branch down to the root.
    pub stack: Vec<BranchStackEntry>,
}

#[derive(Serialize, JsonSchema)]
pub struct BranchStackEntry {
    pub branch: String,
    pub jira_issue: Option<String>,
    pub pr_number: Option<u32>,
    pub is_root: bool,
}

/// list_branches
#[derive(Serialize, JsonSchema)]
pub struct ListBranchesResponse {
    pub branches: Vec<BranchSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct BranchSummary {
    pub branch: String,
    pub jira_issue: Option<String>,
    pub pr_number: Option<u32>,
    pub created_at: String,
}

/// list_repositories
#[derive(Serialize, JsonSchema)]
pub struct ListRepositoriesResponse {
    pub repositories: Vec<RepositorySummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct RepositorySummary {
    pub name: String,
    pub path: String,
    pub last_fetch: Option<String>,
}

/// get_worktrees
#[derive(Serialize, JsonSchema)]
pub struct ListWorktreesResponse {
    pub worktrees: Vec<WorktreeSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct WorktreeSummary {
    pub name: String,
    pub path: String,
    pub branch: String,
}
```

#### GitHub responses

```rust
/// get_pull_request
#[derive(Serialize, JsonSchema)]
pub struct PullRequestResponse {
    pub number: u32,
    pub title: String,
    pub url: String,
    pub state: String,
    pub author: String,
    pub base_branch: String,
    pub head_branch: String,
    pub draft: bool,
    pub mergeable: Option<bool>,
    pub mergeable_state: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub merged_at: Option<String>,
}

/// get_pr_status
#[derive(Serialize, JsonSchema)]
pub struct PullRequestStatusResponse {
    pub pr: PullRequestResponse,
    pub reviews: Vec<ReviewSummary>,
    pub check_runs: Vec<CheckRunSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct ReviewSummary {
    pub reviewer: String,
    pub state: String,
    pub submitted_at: String,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckRunSummary {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub details_url: Option<String>,
}

/// list_pull_requests
#[derive(Serialize, JsonSchema)]
pub struct ListPullRequestsResponse {
    pub pull_requests: Vec<PullRequestSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct PullRequestSummary {
    pub number: u32,
    pub title: String,
    pub url: String,
    pub state: String,
    pub author: String,
    pub draft: bool,
}
```

#### Jira responses

```rust
/// get_jira_issue
#[derive(Serialize, JsonSchema)]
pub struct JiraIssueResponse {
    pub key: String,
    pub summary: String,
    pub description: Option<String>,
    pub status: String,
    pub assignee: Option<String>,
    pub updated: String,
}

/// list_jira_issues
#[derive(Serialize, JsonSchema)]
pub struct ListJiraIssuesResponse {
    pub issues: Vec<JiraIssueSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct JiraIssueSummary {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub assignee: Option<String>,
}
```

These response types live in `twig-mcp/src/responses.rs` (or split per module alongside the tool implementations). Each tool handler constructs the response struct and serializes it with `serde_json::to_string_pretty`. The MCP text content block contains this JSON string. Clients can `JSON.parse()` the text content to access individual fields.

### Standardized Error Format

All tools use a consistent error payload so MCP clients can detect and handle errors uniformly. When a tool encounters an error, it returns the MCP `CallToolResult` with `is_error: true` and the text content set to a JSON object with the following shape:

```rust
#[derive(Serialize, JsonSchema)]
pub struct ToolError {
    /// Machine-readable error code for programmatic handling.
    pub code: ToolErrorCode,
    /// Human-readable description of what went wrong.
    pub message: String,
    /// Optional actionable suggestion for the user or client.
    pub hint: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub enum ToolErrorCode {
    /// No git repository detected (twig-mcp started outside a repo).
    NoRepository,
    /// The .twig/state.json file is missing or unreadable.
    NoTwigState,
    /// The requested branch does not exist in twig state.
    BranchNotFound,
    /// The requested branch has no linked PR.
    NoPullRequest,
    /// The requested branch has no linked Jira issue.
    NoJiraIssue,
    /// GitHub credentials missing from ~/.netrc.
    GitHubAuthMissing,
    /// Jira credentials or config missing.
    JiraAuthMissing,
    /// Network request to GitHub API failed.
    GitHubApiError,
    /// Network request to Jira API failed.
    JiraApiError,
    /// A required parameter was invalid or missing.
    InvalidParameter,
}
```

Example error responses:

```json
{
  "code": "NoRepository",
  "message": "No git repository detected. twig-mcp was started outside a git repository.",
  "hint": "Start twig-mcp from within a git repository, or pass --repo <path>."
}
```

```json
{
  "code": "GitHubAuthMissing",
  "message": "GitHub credentials not found in ~/.netrc.",
  "hint": "Run `twig gh auth` to configure GitHub authentication."
}
```

```json
{
  "code": "BranchNotFound",
  "message": "Branch 'feature/foo' is not tracked by twig.",
  "hint": "Run `twig track feature/foo` to start tracking this branch."
}
```

The `ToolErrorCode` enum is serialized as a string (serde `rename_all` or default variant names) so clients can match on it without parsing nested structures. The `hint` field is optional — omitted when no actionable advice applies.

Implementation helper:

```rust
impl ToolError {
    fn into_call_tool_result(self) -> CallToolResult {
        CallToolResult::error(vec![Content::text(
            serde_json::to_string_pretty(&self).expect("ToolError is always serializable"),
        )])
    }
}
```

### Context Availability Matrix

Since `twig-mcp` may run outside a repository or with partial configuration, each tool has defined behavior for every context state. The server always starts and lists all tools regardless of context — tool availability is not conditional. Instead, tools return structured `ToolError` responses when their required context is missing.

#### Context states

| Context | How detected | Affected tools |
|---------|-------------|----------------|
| **No git repository** | `twig_core::git::detect_repository()` returns `None` at startup | All local state tools, GitHub tools that default to current branch |
| **No twig state** | `.twig/state.json` missing or unreadable | All local state tools except `list_repositories` |
| **No GitHub credentials** | `~/.netrc` has no `github.com` entry | `get_pull_request`, `get_pr_status`, `list_pull_requests` |
| **No Jira config** | Jira config or credentials missing | `get_jira_issue`, `list_jira_issues` |

#### Per-tool behavior matrix

| Tool | No repo | No twig state | No GH creds | No Jira config |
|------|---------|---------------|-------------|----------------|
| `get_current_branch` | `NoRepository` error | Returns branch name only (from git), `jira_issue`/`pr_number` null | OK | OK |
| `get_branch_metadata` | `NoRepository` error | `NoTwigState` error | OK | OK |
| `get_branch_tree` | `NoRepository` error | `NoTwigState` error | OK | OK |
| `get_branch_stack` | `NoRepository` error | `NoTwigState` error | OK | OK |
| `list_branches` | `NoRepository` error | `NoTwigState` error (empty list alternative: see note) | OK | OK |
| `list_repositories` | **OK** — reads global registry, not repo-local state | **OK** | OK | OK |
| `get_worktrees` | `NoRepository` error | `NoTwigState` error | OK | OK |
| `get_pull_request` | `NoPullRequest` error if no `pr_number` param (can't infer from branch) | OK if `pr_number` param provided | `GitHubAuthMissing` error | OK |
| `get_pr_status` | `NoPullRequest` error if no `pr_number` param | OK if `pr_number` param provided | `GitHubAuthMissing` error | OK |
| `list_pull_requests` | `NoRepository` error (can't determine remote) | OK (remote is determined from git, not twig state) | `GitHubAuthMissing` error | OK |
| `get_jira_issue` | `NoJiraIssue` error if no `issue_key` param (can't infer from branch) | OK if `issue_key` param provided | OK | `JiraAuthMissing` error |
| `list_jira_issues` | **OK** — `project` param is required, no repo needed | **OK** | OK | `JiraAuthMissing` error |

**Key design principles:**

- **Always list all tools.** The MCP `tools/list` response includes every tool regardless of current context. This lets clients know what's available and provide appropriate parameters. Hiding tools based on context would make the server's capabilities unpredictable.
- **Explicit parameters bypass context requirements.** GitHub and Jira tools that accept explicit parameters (`pr_number`, `issue_key`, `project`) work without a repository context. Only the "default to current branch" inference path requires a repo.
- **Degrade to partial data, not errors, where possible.** `get_current_branch` returns the branch name from git even if twig state is unreadable — the `jira_issue` and `pr_number` fields are simply null. This is more useful than failing entirely.
- **`list_repositories` is context-free.** It reads the global registry (`${XDG_DATA_HOME}/twig/registry.json`) and never requires a repository or network credentials.

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
│       ├── responses.rs      # Structured response types (CurrentBranchResponse, etc.)
│       ├── errors.rs         # ToolError, ToolErrorCode, error helpers
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

Using `rmcp` macros, each tool is a method on the server struct. The `#[tool(tool_box)]` macro is applied to both the impl block (defining tools) and the `ServerHandler` impl (wiring up protocol handling).

Tools return `CallToolResult` directly (not `String`), using helper methods to produce either a structured JSON success payload or a structured error payload:

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

/// Helper to build a successful CallToolResult from any serializable response.
fn success_result<T: Serialize>(response: &T) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(response).expect("response is always serializable"),
    )])
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
    async fn get_current_branch(&self) -> CallToolResult {
        let repo_path = self.context.require_repo()?;

        let branch = twig_core::git::current_branch();
        let Ok(Some(branch_name)) = branch else {
            return ToolError {
                code: ToolErrorCode::NoRepository,
                message: "Not on any branch (detached HEAD state)".into(),
                hint: Some("Check out a branch first.".into()),
            }.into_call_tool_result();
        };

        // Load fresh state on every call — degrade gracefully if unavailable.
        let (jira_issue, pr_number, created_at) =
            match self.context.load_repo_state() {
                Ok(state) => {
                    let meta = state.branches.get(&branch_name);
                    (
                        meta.and_then(|m| m.jira_issue.clone()),
                        meta.and_then(|m| m.github_pr),
                        meta.map(|m| m.created_at.clone()),
                    )
                }
                Err(_) => (None, None, None),
            };

        success_result(&CurrentBranchResponse {
            branch: branch_name,
            jira_issue,
            pr_number,
            created_at,
        })
    }

    #[tool(
        description = "Get metadata for a specific branch (Jira issue, PR number, creation date)",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn get_branch_metadata(
        &self,
        #[tool(aggr)] params: BranchMetadataParams,
    ) -> CallToolResult {
        let _repo_path = self.context.require_repo()?;
        let state = self.context.load_repo_state().map_err(|_| {
            ToolError::no_twig_state()
        })?;

        let branch_name = params.branch
            .or_else(|| twig_core::git::current_branch().ok().flatten())
            .ok_or_else(|| ToolError {
                code: ToolErrorCode::BranchNotFound,
                message: "No branch specified and could not detect current branch.".into(),
                hint: Some("Provide the `branch` parameter explicitly.".into()),
            })?;

        let meta = state.branches.get(&branch_name).ok_or_else(|| ToolError {
            code: ToolErrorCode::BranchNotFound,
            message: format!("Branch '{branch_name}' is not tracked by twig."),
            hint: Some(format!("Run `twig track {branch_name}` to start tracking this branch.")),
        })?;

        let parents = state.dependency_parents_index
            .get(&branch_name).cloned().unwrap_or_default();
        let children = state.dependency_children_index
            .get(&branch_name).cloned().unwrap_or_default();

        success_result(&BranchMetadataResponse {
            branch: branch_name,
            jira_issue: meta.jira_issue.clone(),
            pr_number: meta.github_pr,
            created_at: meta.created_at.clone(),
            parents,
            children,
        })
    }

    // ... more tools follow the same pattern: return CallToolResult,
    // use success_result() for structured data, ToolError for errors.
}

#[tool(tool_box)]
impl ServerHandler for TwigMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Twig MCP server. Provides read-only access to branch metadata, \
                 Jira issues, and GitHub PRs for the current repository. \
                 All tool responses are JSON objects. Error responses have \
                 {code, message, hint} fields.".into()
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
