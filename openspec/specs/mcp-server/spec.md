# MCP Server

## Purpose

Expose twig capabilities to AI assistants (Claude Code, etc.) via the Model Context Protocol. Provides 12 read-only
tools: 7 local state tools, 3 GitHub tools, and 2 Jira tools. Runs as a standalone binary (`twig-mcp`) communicating
over stdio transport.

**Binary:** `twig-mcp` **Crates:** `twig-mcp` (server, context, tools, types) **Dependencies:** `rmcp` v0.15,
`twig-core`, `twig-gh`, `twig-jira`

## Server Lifecycle

### Requirement: Server startup and transport

#### Scenario: Starting the MCP server

WHEN the `twig-mcp` binary is executed THEN it parses CLI arguments (`--verbose` with 0–3 verbosity levels and optional
`--repo PATH`) AND initializes tracing to stderr at the appropriate level (WARN, INFO, DEBUG, or TRACE) AND loads
`ConfigDirs` from XDG-standard paths AND creates a `ServerContext` with the resolved configuration AND starts the MCP
server on stdio transport using JSON-RPC AND blocks on `service.waiting()` until the client disconnects

#### Scenario: Repository detection at startup

WHEN `twig-mcp` is started without `--repo` THEN it auto-detects the repository via `detect_repository()` from the
current working directory

WHEN `twig-mcp` is started with `--repo PATH` THEN it uses the provided path as the repository location instead of
auto-detecting

#### Scenario: Server capabilities advertised to clients

WHEN a client queries server capabilities via the MCP `initialize` handshake THEN the server responds with
`enable_tools()` and `enable_prompts()` AND the instructions field reads "Twig MCP server. Provides read-only access to
branch metadata, Jira issues, and GitHub PRs for the current repository."

## Server Context

### Requirement: Repository context resolution

#### Scenario: Repository is available

WHEN a tool calls `require_repo()` AND a repository path was detected at startup THEN the resolved path is returned

#### Scenario: Repository is not available

WHEN a tool calls `require_repo()` AND no repository was detected THEN the tool returns a structured error with
code `no_repo`, message "twig-mcp was started outside a git repository", and hint "Run twig-mcp from within a git
repository."

### Requirement: Twig state loading

#### Scenario: Loading repo state successfully

WHEN a tool calls `load_repo_state()` THEN it opens the git repository at the repo path AND loads `RepoState` from
`.twig/state.json` AND returns a fresh state on each call (state is not cached)

#### Scenario: Twig state is missing or unreadable

WHEN a tool calls `require_repo_state()` AND `.twig/state.json` does not exist or cannot be loaded THEN the tool returns
a structured error with code `no_twig_state`, a message including the underlying cause, and hint "Run `twig init` in
this repository first."

### Requirement: Lazy GitHub client initialization

#### Scenario: First GitHub tool invocation

WHEN a GitHub tool is called for the first time THEN the server calls
`twig_gh::create_github_client_from_netrc(&home_dir)` AND stores the client in a `tokio::sync::OnceCell` for reuse

#### Scenario: Subsequent GitHub tool invocations

WHEN a GitHub tool is called after the client has been initialized THEN the cached client is returned immediately without
re-initialization

#### Scenario: GitHub credentials are missing

WHEN GitHub client initialization fails THEN the tool returns a structured error with code `credentials_missing`,
message "GitHub credentials not found", and hint "Add credentials for github.com to `~/.netrc`. See `twig auth --help`."

### Requirement: Lazy Jira client initialization

#### Scenario: First Jira tool invocation

WHEN a Jira tool is called for the first time THEN the server reads `$JIRA_HOST` to determine the Jira hostname AND
calls `twig_jira::create_jira_client_from_netrc(&home_dir, &host)` AND stores the `(JiraClient, String)` tuple in a
`tokio::sync::OnceCell` for reuse

#### Scenario: Jira credentials or host are missing

WHEN Jira client initialization fails (missing `$JIRA_HOST` or netrc entry) THEN the tool returns a structured error
with code `credentials_missing`, message "Jira credentials not found", and hint "Set $JIRA_HOST and add credentials to
`~/.netrc`. See `twig auth --help`."

### Requirement: GitHub repository extraction from remote

#### Scenario: Origin remote points to GitHub

WHEN `get_github_repo()` is called AND the `origin` remote exists with a parseable GitHub URL THEN the owner and
repository name are extracted and returned as a `GitHubRepo`

#### Scenario: Origin remote is missing

WHEN `get_github_repo()` is called AND no remote named `origin` exists THEN the tool returns a structured error with
code `not_found` and hint "Add a GitHub remote named 'origin'."

#### Scenario: Remote URL is not a GitHub repository

WHEN `get_github_repo()` is called AND the `origin` URL cannot be parsed as a GitHub repository THEN the tool returns a
structured error with code `not_found` and hint "Ensure the 'origin' remote points to a GitHub repository."

## Response Envelope

### Requirement: Structured response format

#### Scenario: Successful tool response

WHEN any tool completes successfully THEN the response is serialized as JSON with `status: "ok"` and `data` containing
the tool-specific response AND the `is_error` flag is set to `false`

#### Scenario: Error tool response

WHEN any tool encounters an error THEN the response is serialized as JSON with `status: "error"` and a nested error
object containing `code` (machine-readable string), `message` (human-readable explanation), and optionally `hint`
(actionable guidance) AND the `is_error` flag is set to `true` AND the `hint` field is omitted from serialization when
not present

## Local State Tools

All local state tools are annotated with `read_only_hint = true` and `idempotent_hint = true`.

### Requirement: Get current branch

#### Scenario: On a branch with linked metadata

WHEN `get_current_branch` is called AND HEAD points to a branch THEN the tool returns a `BranchMetadataResponse` with
the branch name and any linked `jira_issue`, `pr_number`, `parent_branch`, and `created_at` from the twig state AND
fields with no value are omitted from the JSON response

#### Scenario: Detached HEAD state

WHEN `get_current_branch` is called AND HEAD is detached THEN the tool returns a structured error with code `not_found`
and message "Not on any branch (detached HEAD state)"

#### Scenario: Twig state is unavailable

WHEN `get_current_branch` is called AND the twig state cannot be loaded THEN the tool still returns the branch name AND
metadata fields are omitted (graceful degradation)

### Requirement: Get branch metadata

Parameters: `branch` (string, required)

#### Scenario: Branch exists in twig state

WHEN `get_branch_metadata` is called with a branch name that exists in `state.branches` THEN the tool returns a
`BranchMetadataResponse` with the branch's `jira_issue`, `pr_number`, `parent_branch`, and `created_at`

#### Scenario: Branch not tracked in twig state

WHEN `get_branch_metadata` is called with a branch name not found in `state.branches` THEN the tool returns a
structured error with code `not_found`, message "Branch '{branch}' not found in twig state", and hint "Use
`list_branches` to see tracked branches."

### Requirement: Get branch tree

Parameters: `branch` (optional string, defaults to root)

#### Scenario: Tree built from dependency graph

WHEN `get_branch_tree` is called THEN the tool opens the git repository AND builds a `BranchGraph` with declared
dependencies and orphan parenting enabled AND recursively constructs a tree of `BranchTreeNode` objects from the
resolved root AND renders the tree as text using box-drawing characters (`├──`, `└──`, `│`) AND returns a
`BranchTreeResponse` with `root`, `tree_text`, and `branches` fields

#### Scenario: Explicit root branch specified

WHEN `get_branch_tree` is called with a `branch` parameter THEN that branch is used as the tree root instead of the
default root candidate

#### Scenario: No branches in repository

WHEN `get_branch_tree` is called AND the graph contains no branches THEN the tool returns a structured error with
code `not_found` and message "No branches found in repository"

#### Scenario: No root branch found

WHEN `get_branch_tree` is called without a `branch` parameter AND no root candidates exist in the graph THEN the tool
returns a structured error with code `not_found` and message "No root branch found in repository"

### Requirement: Get branch stack

Parameters: `branch` (optional string, defaults to current)

#### Scenario: Walking the dependency chain to root

WHEN `get_branch_stack` is called THEN the tool determines the start branch (from the parameter or the current branch)
AND walks up the dependency chain using `state.get_dependency_parents()`, following the first parent at each step AND
maintains a `HashSet` for cycle protection AND returns a `BranchStackResponse` with a stack vector ordered from the start
branch (index 0) to the root (last element) AND each entry includes full branch metadata

#### Scenario: Detached HEAD with no branch parameter

WHEN `get_branch_stack` is called without a `branch` parameter AND HEAD is detached THEN the tool returns a structured
error with code `not_found` and message "Not on any branch (detached HEAD)"

### Requirement: List branches

#### Scenario: Listing all tracked branches

WHEN `list_branches` is called THEN the tool iterates over all keys in `state.branches` AND returns a
`ListBranchesResponse` containing a `BranchMetadataResponse` for each branch with its linked metadata

### Requirement: List repositories

#### Scenario: Loading the global registry

WHEN `list_repositories` is called THEN the tool loads the global `Registry` from `${XDG_DATA_HOME}/twig/registry.json`
AND returns a `ListRepositoriesResponse` with each repository's `name` and `path` AND this tool does not require a
repository context (it reads the global registry)

#### Scenario: Registry load fails

WHEN `list_repositories` is called AND the registry cannot be loaded THEN the tool returns a structured error with
code `internal` and message "Failed to load registry: {e}"

### Requirement: Get worktrees

#### Scenario: Listing worktrees from twig state

WHEN `get_worktrees` is called THEN the tool iterates over `state.worktrees` AND returns a `ListWorktreesResponse` with
each worktree's `name`, `path`, and `branch`

## GitHub Tools

All GitHub tools are annotated with `read_only_hint = true`. They are not annotated as idempotent because PR state can
change between calls.

### Requirement: PR number resolution

This resolution is shared by `get_pull_request` and `get_pr_status`.

#### Scenario: PR number provided explicitly

WHEN a GitHub tool is called with a `pr_number` parameter THEN that number is used directly

#### Scenario: PR number resolved from current branch

WHEN a GitHub tool is called without a `pr_number` parameter THEN the tool looks up the current branch name AND finds
the linked `github_pr` from the twig state

#### Scenario: No PR linked to current branch

WHEN a GitHub tool is called without a `pr_number` parameter AND the current branch has no linked PR THEN the tool
returns a structured error with code `not_found`, message "Branch '{branch}' has no linked GitHub PR", and hint "Provide
an explicit pr_number parameter."

### Requirement: Get pull request

Parameters: `pr_number` (optional u32)

#### Scenario: Fetching PR details

WHEN `get_pull_request` is called AND the PR number is resolved THEN the tool calls the GitHub API via
`gh.get_pull_request(&owner, &repo, pr_number)` AND returns a `PullRequestResponse` with `number`, `title`, `state`,
`author`, `base`, `head`, `draft`, `mergeable`, `created_at`, and `updated_at`

#### Scenario: GitHub API error

WHEN the GitHub API call fails THEN the tool returns a structured error with code `network_error` and message "GitHub
API error: {e}"

### Requirement: Get PR status

Parameters: `pr_number` (optional u32)

#### Scenario: Fetching PR with reviews and checks

WHEN `get_pr_status` is called AND the PR number is resolved THEN the tool calls
`gh.get_pr_status(&owner, &repo, pr_number)` AND returns a `PrStatusResponse` with the `pull_request` details, a list
of `reviews` (each with `author` and `state`), and a list of `checks` (each with `name`, `status`, and optional
`conclusion`)

### Requirement: List pull requests

Parameters: `state` (optional string, defaults to "open")

#### Scenario: Listing PRs by state

WHEN `list_pull_requests` is called THEN the tool calls `gh.list_pull_requests(&owner, &repo, state, None)` AND accepts
state values "open", "closed", or "all" AND returns a `ListPullRequestsResponse` containing a `PullRequestResponse` for
each PR

## Jira Tools

All Jira tools are annotated with `read_only_hint = true`.

### Requirement: Jira issue key resolution

This resolution is shared by `get_jira_issue`.

#### Scenario: Issue key provided explicitly

WHEN a Jira tool is called with an `issue_key` parameter THEN that key is used directly

#### Scenario: Issue key resolved from current branch

WHEN a Jira tool is called without an `issue_key` parameter THEN the tool looks up the current branch name AND finds
the linked `jira_issue` from the twig state

#### Scenario: No issue linked to current branch

WHEN a Jira tool is called without an `issue_key` parameter AND the current branch has no linked Jira issue THEN the
tool returns a structured error with code `not_found`, message "Branch '{branch}' has no linked Jira issue", and hint
"Provide an explicit issue_key parameter."

#### Scenario: Detached HEAD with no issue key

WHEN a Jira tool is called without an `issue_key` parameter AND HEAD is detached THEN the tool returns a structured
error with code `invalid_params`, message "No issue_key provided and could not detect current branch", and hint "Provide
an explicit issue_key parameter."

### Requirement: Get Jira issue

Parameters: `issue_key` (optional string)

#### Scenario: Fetching issue details

WHEN `get_jira_issue` is called AND the issue key is resolved THEN the tool calls `jira.get_issue(&issue_key)` AND
returns a `JiraIssueResponse` with `key`, `summary`, `status` (from `status.name`), and optional `description` and
`assignee` (from `assignee.display_name`) AND optional fields are omitted from JSON when not present

#### Scenario: Jira API error

WHEN the Jira API call fails THEN the tool returns a structured error with code `network_error` and message "Jira API
error: {e}"

### Requirement: List Jira issues

Parameters: `project` (required string), `status` (optional string), `assignee` (optional string, "me" for current
user)

#### Scenario: Listing issues with filters

WHEN `list_jira_issues` is called THEN the tool calls `jira.list_issues()` with the provided `project`, optional
`status`, optional `assignee`, and no limit AND returns a `ListJiraIssuesResponse` containing a `JiraIssueResponse` for
each issue

## Prompts

### Requirement: Stack status prompt

#### Scenario: Summarizing the PR stack

WHEN the `stack-status` prompt is called THEN the server collects the current branch, all branch dependency trees (from
all root candidates), and per-branch details (sorted alphabetically) AND for each branch shows name, linked Jira issue,
PR number, and parent AND renders trees using box-drawing characters AND returns a single `PromptMessage` with role
`User` ending with "Summarize the current state of this PR stack..."

#### Scenario: No repository data available

WHEN the `stack-status` prompt is called AND no repository or state is available THEN the prompt returns "No repository
or branch data available."

### Requirement: Branch context prompt

Parameters: `branch` (optional string, defaults to current)

#### Scenario: Describing the current branch context

WHEN the `branch-context` prompt is called AND a branch is resolved (from parameter or current branch) THEN the server
collects the branch name, parent branch, linked Jira issue, linked PR number, and creation date AND returns a single
`PromptMessage` with role `User` ending with "Describe what I'm working on based on this branch context..."

#### Scenario: Not on a git branch

WHEN the `branch-context` prompt is called without a branch parameter AND HEAD is detached or no repository is detected
THEN the prompt returns "I'm not currently on a git branch. What should I work on?"
