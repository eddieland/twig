//! MCP server implementation with all tool handlers.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use twig_core::git::graph::{BranchGraph, BranchGraphBuilder, BranchName};
use twig_core::state::{Registry, RepoState};

use crate::context::ServerContext;
use crate::tools::github::{GetPrStatusParams, GetPullRequestParams, ListPullRequestsParams};
use crate::tools::jira::{GetJiraIssueParams, ListJiraIssuesParams};
use crate::tools::local::{BranchMetadataParams, BranchStackParams, BranchTreeParams};
use crate::types::*;

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

  // =========================================================================
  // Local state tools
  // =========================================================================

  #[tool(
    description = "Get the current git branch name and its linked Jira issue and GitHub PR",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn get_current_branch(&self) -> Result<CallToolResult, McpError> {
    let repo_path = match self.context.require_repo() {
      Ok(p) => p,
      Err(e) => return e.into_result(),
    };

    let branch_name = match get_current_branch_name(repo_path) {
      Ok(Some(name)) => name,
      Ok(None) => {
        return Ok(
          ToolResponse::<BranchMetadataResponse>::err("not_found", "Not on any branch (detached HEAD state)", None)
            .to_call_tool_result(),
        );
      }
      Err(e) => {
        return Ok(
          ToolResponse::<BranchMetadataResponse>::err("internal", format!("Failed to get current branch: {e}"), None)
            .to_call_tool_result(),
        );
      }
    };

    // Degrade gracefully if state is missing
    let (jira_issue, pr_number, parent_branch, created_at) = match self.context.load_repo_state() {
      Ok(state) => extract_branch_metadata(&state, &branch_name),
      Err(_) => (None, None, None, None),
    };

    Ok(
      ToolResponse::ok(BranchMetadataResponse {
        branch: branch_name,
        jira_issue,
        pr_number,
        parent_branch,
        created_at,
      })
      .to_call_tool_result(),
    )
  }

  #[tool(
    description = "Get metadata for a specific branch (Jira issue, PR number, parent branch)",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn get_branch_metadata(&self, params: Parameters<BranchMetadataParams>) -> Result<CallToolResult, McpError> {
    let _repo_path = match self.context.require_repo() {
      Ok(p) => p,
      Err(e) => return e.into_result(),
    };
    let state = match self.context.require_repo_state() {
      Ok(s) => s,
      Err(e) => return e.into_result(),
    };

    let branch_name = &params.0.branch;
    if !state.branches.contains_key(branch_name) {
      return Ok(
        ToolResponse::<BranchMetadataResponse>::err(
          "not_found",
          format!("Branch '{branch_name}' not found in twig state"),
          Some("Use `list_branches` to see tracked branches.".into()),
        )
        .to_call_tool_result(),
      );
    }

    let (jira_issue, pr_number, parent_branch, created_at) = extract_branch_metadata(&state, branch_name);

    Ok(
      ToolResponse::ok(BranchMetadataResponse {
        branch: branch_name.clone(),
        jira_issue,
        pr_number,
        parent_branch,
        created_at,
      })
      .to_call_tool_result(),
    )
  }

  #[tool(
    description = "Get the branch dependency tree for the current repository",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn get_branch_tree(&self, params: Parameters<BranchTreeParams>) -> Result<CallToolResult, McpError> {
    let repo_path = match self.context.require_repo() {
      Ok(p) => p,
      Err(e) => return e.into_result(),
    };

    let repo = match git2::Repository::open(repo_path) {
      Ok(r) => r,
      Err(e) => {
        return Ok(
          ToolResponse::<BranchTreeResponse>::err("internal", format!("Failed to open repository: {e}"), None)
            .to_call_tool_result(),
        );
      }
    };

    let graph = match BranchGraphBuilder::new()
      .with_declared_dependencies(true)
      .with_orphan_parenting(true)
      .build(&repo)
    {
      Ok(g) => g,
      Err(e) => {
        return Ok(
          ToolResponse::<BranchTreeResponse>::err("internal", format!("Failed to build branch graph: {e}"), None)
            .to_call_tool_result(),
        );
      }
    };

    if graph.is_empty() {
      return Ok(
        ToolResponse::<BranchTreeResponse>::err("not_found", "No branches found in repository", None)
          .to_call_tool_result(),
      );
    }

    let state = self.context.load_repo_state().unwrap_or_default();

    // Determine root
    let root_name = if let Some(ref branch) = params.0.branch {
      branch.clone()
    } else {
      match graph.root_candidates().first() {
        Some(b) => b.as_str().to_string(),
        None => {
          return Ok(
            ToolResponse::<BranchTreeResponse>::err("not_found", "No root branch found in repository", None)
              .to_call_tool_result(),
          );
        }
      }
    };

    let root_branch_name = BranchName::from(root_name.as_str());
    let tree_nodes = build_tree_node(&graph, &state, &root_branch_name);
    let mut text = String::new();
    render_tree_text(&tree_nodes, &mut text, "", true);

    Ok(
      ToolResponse::ok(BranchTreeResponse {
        root: root_name,
        tree_text: text,
        branches: vec![tree_nodes],
      })
      .to_call_tool_result(),
    )
  }

  #[tool(
    description = "Get the ancestor chain (stack) from a branch up to its root",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn get_branch_stack(&self, params: Parameters<BranchStackParams>) -> Result<CallToolResult, McpError> {
    let repo_path = match self.context.require_repo() {
      Ok(p) => p,
      Err(e) => return e.into_result(),
    };
    let state = match self.context.require_repo_state() {
      Ok(s) => s,
      Err(e) => return e.into_result(),
    };

    let start_branch = match &params.0.branch {
      Some(b) => b.clone(),
      None => match get_current_branch_name(repo_path) {
        Ok(Some(name)) => name,
        Ok(None) => {
          return Ok(
            ToolResponse::<BranchStackResponse>::err("not_found", "Not on any branch (detached HEAD)", None)
              .to_call_tool_result(),
          );
        }
        Err(e) => {
          return Ok(
            ToolResponse::<BranchStackResponse>::err("internal", format!("Failed to get current branch: {e}"), None)
              .to_call_tool_result(),
          );
        }
      },
    };

    // Walk up the dependency chain
    let mut stack = Vec::new();
    let mut current = start_branch;
    let mut visited = std::collections::HashSet::new();

    loop {
      if !visited.insert(current.clone()) {
        break; // Cycle protection
      }

      let (jira_issue, pr_number, parent_branch, created_at) = extract_branch_metadata(&state, &current);

      let parent = parent_branch.clone();
      stack.push(BranchMetadataResponse {
        branch: current,
        jira_issue,
        pr_number,
        parent_branch,
        created_at,
      });

      match parent {
        Some(p) => current = p,
        None => break,
      }
    }

    Ok(ToolResponse::ok(BranchStackResponse { stack }).to_call_tool_result())
  }

  #[tool(
    description = "List all twig-tracked branches in the current repository",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn list_branches(&self) -> Result<CallToolResult, McpError> {
    let _repo_path = match self.context.require_repo() {
      Ok(p) => p,
      Err(e) => return e.into_result(),
    };
    let state = match self.context.require_repo_state() {
      Ok(s) => s,
      Err(e) => return e.into_result(),
    };

    let branches: Vec<BranchMetadataResponse> = state
      .branches
      .keys()
      .map(|name| {
        let (jira_issue, pr_number, parent_branch, created_at) = extract_branch_metadata(&state, name);
        BranchMetadataResponse {
          branch: name.clone(),
          jira_issue,
          pr_number,
          parent_branch,
          created_at,
        }
      })
      .collect();

    Ok(ToolResponse::ok(ListBranchesResponse { branches }).to_call_tool_result())
  }

  #[tool(
    description = "List all twig-registered repositories",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn list_repositories(&self) -> Result<CallToolResult, McpError> {
    let registry = match Registry::load(&self.context.config_dirs) {
      Ok(r) => r,
      Err(e) => {
        return Ok(
          ToolResponse::<ListRepositoriesResponse>::err("internal", format!("Failed to load registry: {e}"), None)
            .to_call_tool_result(),
        );
      }
    };

    let repositories: Vec<RepositoryInfo> = registry
      .list()
      .iter()
      .map(|r| RepositoryInfo {
        name: r.name.clone(),
        path: r.path.clone(),
      })
      .collect();

    Ok(ToolResponse::ok(ListRepositoriesResponse { repositories }).to_call_tool_result())
  }

  #[tool(
    description = "Get active worktrees for the current repository",
    annotations(read_only_hint = true, idempotent_hint = true)
  )]
  async fn get_worktrees(&self) -> Result<CallToolResult, McpError> {
    let _repo_path = match self.context.require_repo() {
      Ok(p) => p,
      Err(e) => return e.into_result(),
    };
    let state = match self.context.require_repo_state() {
      Ok(s) => s,
      Err(e) => return e.into_result(),
    };

    let worktrees: Vec<WorktreeInfo> = state
      .worktrees
      .iter()
      .map(|w| WorktreeInfo {
        name: w.name.clone(),
        path: w.path.clone(),
        branch: w.branch.clone(),
      })
      .collect();

    Ok(ToolResponse::ok(ListWorktreesResponse { worktrees }).to_call_tool_result())
  }

  // =========================================================================
  // GitHub tools
  // =========================================================================

  #[tool(
    description = "Get full details for a GitHub pull request. Defaults to the current branch's PR.",
    annotations(read_only_hint = true)
  )]
  async fn get_pull_request(&self, params: Parameters<GetPullRequestParams>) -> Result<CallToolResult, McpError> {
    let gh = match self.context.get_github_client().await {
      Ok(c) => c,
      Err(e) => return e.into_result(),
    };
    let gh_repo = match self.context.get_github_repo() {
      Ok(r) => r,
      Err(e) => return e.into_result(),
    };

    let pr_number = match resolve_pr_number(&self.context, params.0.pr_number) {
      Ok(n) => n,
      Err(e) => return e.into_result(),
    };

    match gh.get_pull_request(&gh_repo.owner, &gh_repo.repo, pr_number).await {
      Ok(pr) => Ok(ToolResponse::ok(map_pull_request(&pr)).to_call_tool_result()),
      Err(e) => Ok(
        ToolResponse::<PullRequestResponse>::err("network_error", format!("GitHub API error: {e}"), None)
          .to_call_tool_result(),
      ),
    }
  }

  #[tool(
    description = "Get PR details with reviews and CI check status. Defaults to the current branch's PR.",
    annotations(read_only_hint = true)
  )]
  async fn get_pr_status(&self, params: Parameters<GetPrStatusParams>) -> Result<CallToolResult, McpError> {
    let gh = match self.context.get_github_client().await {
      Ok(c) => c,
      Err(e) => return e.into_result(),
    };
    let gh_repo = match self.context.get_github_repo() {
      Ok(r) => r,
      Err(e) => return e.into_result(),
    };

    let pr_number = match resolve_pr_number(&self.context, params.0.pr_number) {
      Ok(n) => n,
      Err(e) => return e.into_result(),
    };

    let status = match gh.get_pr_status(&gh_repo.owner, &gh_repo.repo, pr_number).await {
      Ok(s) => s,
      Err(e) => {
        return Ok(
          ToolResponse::<PrStatusResponse>::err("network_error", format!("GitHub API error: {e}"), None)
            .to_call_tool_result(),
        );
      }
    };

    let reviews: Vec<ReviewInfo> = status
      .reviews
      .iter()
      .map(|r| ReviewInfo {
        author: r.user.login.clone(),
        state: r.state.clone(),
      })
      .collect();

    let checks: Vec<CheckRunInfo> = status
      .check_runs
      .iter()
      .map(|c| CheckRunInfo {
        name: c.name.clone(),
        status: c.status.clone(),
        conclusion: c.conclusion.clone(),
      })
      .collect();

    Ok(
      ToolResponse::ok(PrStatusResponse {
        pull_request: map_pull_request(&status.pr),
        reviews,
        checks,
      })
      .to_call_tool_result(),
    )
  }

  #[tool(
    description = "List pull requests for the current repository. Defaults to open PRs.",
    annotations(read_only_hint = true)
  )]
  async fn list_pull_requests(&self, params: Parameters<ListPullRequestsParams>) -> Result<CallToolResult, McpError> {
    let gh = match self.context.get_github_client().await {
      Ok(c) => c,
      Err(e) => return e.into_result(),
    };
    let gh_repo = match self.context.get_github_repo() {
      Ok(r) => r,
      Err(e) => return e.into_result(),
    };

    let state = params.0.state.as_deref();
    match gh.list_pull_requests(&gh_repo.owner, &gh_repo.repo, state, None).await {
      Ok(prs) => {
        let pull_requests: Vec<PullRequestResponse> = prs.iter().map(map_pull_request).collect();
        Ok(ToolResponse::ok(ListPullRequestsResponse { pull_requests }).to_call_tool_result())
      }
      Err(e) => Ok(
        ToolResponse::<ListPullRequestsResponse>::err("network_error", format!("GitHub API error: {e}"), None)
          .to_call_tool_result(),
      ),
    }
  }

  // =========================================================================
  // Jira tools
  // =========================================================================

  #[tool(
    description = "Get details for a Jira issue. Defaults to the current branch's linked issue.",
    annotations(read_only_hint = true)
  )]
  async fn get_jira_issue(&self, params: Parameters<GetJiraIssueParams>) -> Result<CallToolResult, McpError> {
    let jira = match self.context.get_jira_client().await {
      Ok(c) => c,
      Err(e) => return e.into_result(),
    };

    let issue_key = match resolve_jira_key(&self.context, params.0.issue_key) {
      Ok(k) => k,
      Err(e) => return e.into_result(),
    };

    match jira.get_issue(&issue_key).await {
      Ok(issue) => Ok(ToolResponse::ok(map_jira_issue(&issue)).to_call_tool_result()),
      Err(e) => Ok(
        ToolResponse::<JiraIssueResponse>::err("network_error", format!("Jira API error: {e}"), None)
          .to_call_tool_result(),
      ),
    }
  }

  #[tool(
    description = "List Jira issues for a project with optional status and assignee filters",
    annotations(read_only_hint = true)
  )]
  async fn list_jira_issues(&self, params: Parameters<ListJiraIssuesParams>) -> Result<CallToolResult, McpError> {
    let jira = match self.context.get_jira_client().await {
      Ok(c) => c,
      Err(e) => return e.into_result(),
    };

    let p = &params.0;
    match jira
      .list_issues(
        Some(p.project.as_str()),
        p.status.as_deref(),
        p.assignee.as_deref(),
        None,
      )
      .await
    {
      Ok(issues) => {
        let mapped: Vec<JiraIssueResponse> = issues.iter().map(map_jira_issue).collect();
        Ok(ToolResponse::ok(ListJiraIssuesResponse { issues: mapped }).to_call_tool_result())
      }
      Err(e) => Ok(
        ToolResponse::<ListJiraIssuesResponse>::err("network_error", format!("Jira API error: {e}"), None)
          .to_call_tool_result(),
      ),
    }
  }
}

#[tool_handler]
impl ServerHandler for TwigMcpServer {
  fn get_info(&self) -> ServerInfo {
    ServerInfo {
      instructions: Some(
        "Twig MCP server. Provides read-only access to branch metadata, \
         Jira issues, and GitHub PRs for the current repository."
          .into(),
      ),
      capabilities: ServerCapabilities::builder().enable_tools().build(),
      ..Default::default()
    }
  }
}

// ===========================================================================
// Helper functions
// ===========================================================================

/// Get the current branch name from a repository path.
fn get_current_branch_name(repo_path: &std::path::Path) -> anyhow::Result<Option<String>> {
  let repo = git2::Repository::open(repo_path)?;
  let head = repo.head()?;
  Ok(head.shorthand().map(|s| s.to_string()))
}

/// Extract metadata for a branch from state.
fn extract_branch_metadata(
  state: &RepoState,
  branch_name: &str,
) -> (Option<String>, Option<u32>, Option<String>, Option<String>) {
  let meta = state.branches.get(branch_name);
  let jira_issue = meta.and_then(|m| m.jira_issue.clone());
  let pr_number = meta.and_then(|m| m.github_pr);
  let created_at = meta.map(|m| m.created_at.clone());
  let parent_branch = state
    .get_dependency_parents(branch_name)
    .first()
    .map(|s| (*s).to_string());
  (jira_issue, pr_number, parent_branch, created_at)
}

/// Resolve a PR number from explicit params or current branch state.
fn resolve_pr_number(context: &ServerContext, explicit: Option<u32>) -> Result<u32, ToolError> {
  if let Some(n) = explicit {
    return Ok(n);
  }
  // Try to get from current branch
  let repo_path = context.require_repo()?;
  let branch = get_current_branch_name(repo_path)
    .ok()
    .flatten()
    .ok_or_else(|| ToolError {
      code: "invalid_params".into(),
      message: "No pr_number provided and could not detect current branch".into(),
      hint: Some("Provide an explicit pr_number parameter.".into()),
    })?;
  let state = context.require_repo_state()?;
  state
    .branches
    .get(&branch)
    .and_then(|m| m.github_pr)
    .ok_or_else(|| ToolError {
      code: "not_found".into(),
      message: format!("Branch '{branch}' has no linked GitHub PR"),
      hint: Some("Provide an explicit pr_number parameter.".into()),
    })
}

/// Resolve a Jira issue key from explicit params or current branch state.
fn resolve_jira_key(context: &ServerContext, explicit: Option<String>) -> Result<String, ToolError> {
  if let Some(k) = explicit {
    return Ok(k);
  }
  let repo_path = context.require_repo()?;
  let branch = get_current_branch_name(repo_path)
    .ok()
    .flatten()
    .ok_or_else(|| ToolError {
      code: "invalid_params".into(),
      message: "No issue_key provided and could not detect current branch".into(),
      hint: Some("Provide an explicit issue_key parameter.".into()),
    })?;
  let state = context.require_repo_state()?;
  state
    .branches
    .get(&branch)
    .and_then(|m| m.jira_issue.clone())
    .ok_or_else(|| ToolError {
      code: "not_found".into(),
      message: format!("Branch '{branch}' has no linked Jira issue"),
      hint: Some("Provide an explicit issue_key parameter.".into()),
    })
}

/// Map a GitHub PR to our response type.
fn map_pull_request(pr: &twig_gh::GitHubPullRequest) -> PullRequestResponse {
  PullRequestResponse {
    number: pr.number,
    title: pr.title.clone(),
    state: pr.state.clone(),
    author: pr.user.login.clone(),
    base: pr.base.ref_name.clone().unwrap_or_default(),
    head: pr.head.ref_name.clone().unwrap_or_default(),
    draft: pr.draft.unwrap_or(false),
    mergeable: pr.mergeable,
    created_at: pr.created_at.clone(),
    updated_at: pr.updated_at.clone(),
  }
}

/// Map a Jira issue to our response type.
fn map_jira_issue(issue: &twig_jira::Issue) -> JiraIssueResponse {
  // Jira descriptions can be complex objects in API v3. Convert to string.
  let description = issue.fields.description.as_ref().map(|d| d.to_string());
  JiraIssueResponse {
    key: issue.key.clone(),
    summary: issue.fields.summary.clone(),
    description,
    status: issue.fields.status.name.clone(),
    assignee: issue.fields.assignee.as_ref().map(|a| a.display_name.clone()),
  }
}

/// Recursively build a `BranchTreeNode` from a `BranchGraph`.
fn build_tree_node(graph: &BranchGraph, state: &RepoState, name: &BranchName) -> BranchTreeNode {
  let node = graph.get(name);
  let meta = state.branches.get(name.as_str());

  let children: Vec<BranchTreeNode> = node
    .map(|n| {
      n.topology
        .children
        .iter()
        .map(|child| build_tree_node(graph, state, child))
        .collect()
    })
    .unwrap_or_default();

  BranchTreeNode {
    branch: name.as_str().to_string(),
    children,
    jira_issue: meta.and_then(|m| m.jira_issue.clone()),
    pr_number: meta.and_then(|m| m.github_pr),
  }
}

/// Render a tree node as indented text using box-drawing characters.
fn render_tree_text(node: &BranchTreeNode, out: &mut String, prefix: &str, is_root: bool) {
  if is_root {
    out.push_str(&node.branch);
    // Add metadata inline
    let mut annotations = Vec::new();
    if let Some(ref jira) = node.jira_issue {
      annotations.push(jira.clone());
    }
    if let Some(pr) = node.pr_number {
      annotations.push(format!("#{pr}"));
    }
    if !annotations.is_empty() {
      out.push_str(&format!(" ({})", annotations.join(", ")));
    }
    out.push('\n');
  }

  let child_count = node.children.len();
  for (i, child) in node.children.iter().enumerate() {
    let is_last = i == child_count - 1;
    let connector = if is_last { "└── " } else { "├── " };
    let child_prefix = if is_last { "    " } else { "│   " };

    out.push_str(prefix);
    out.push_str(connector);
    out.push_str(&child.branch);

    // Add metadata inline
    let mut annotations = Vec::new();
    if let Some(ref jira) = child.jira_issue {
      annotations.push(jira.clone());
    }
    if let Some(pr) = child.pr_number {
      annotations.push(format!("#{pr}"));
    }
    if !annotations.is_empty() {
      out.push_str(&format!(" ({})", annotations.join(", ")));
    }
    out.push('\n');

    let new_prefix = format!("{prefix}{child_prefix}");
    render_tree_text(child, out, &new_prefix, false);
  }
}
