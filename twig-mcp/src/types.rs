//! Structured response and error types for twig-mcp tools.
//!
//! Every tool returns a JSON-serialized `ToolResponse<T>` — either an `ok`
//! payload or a structured error with a machine-readable code.

use rmcp::model::{CallToolResult, Content};
use serde::Serialize;

/// Standard envelope for all tool responses.
#[derive(Debug, Serialize)]
#[serde(tag = "status")]
pub enum ToolResponse<T: Serialize> {
  #[serde(rename = "ok")]
  Ok { data: T },
  #[serde(rename = "error")]
  Error { error: ToolError },
}

impl<T: Serialize> ToolResponse<T> {
  pub fn ok(data: T) -> Self {
    Self::Ok { data }
  }

  pub fn err(code: impl Into<String>, message: impl Into<String>, hint: Option<String>) -> Self {
    Self::Error {
      error: ToolError {
        code: code.into(),
        message: message.into(),
        hint,
      },
    }
  }

  /// Serialize to a `CallToolResult`, setting `is_error` for error responses.
  pub fn to_call_tool_result(&self) -> CallToolResult {
    let json = serde_json::to_string(self).unwrap_or_else(|e| {
      format!(r#"{{"status":"error","error":{{"code":"internal","message":"Serialization failed: {e}"}}}}"#)
    });
    let is_error = matches!(self, Self::Error { .. });
    let mut result = CallToolResult::success(vec![Content::text(json)]);
    result.is_error = Some(is_error);
    result
  }
}

/// Consistent error shape returned by all tools.
#[derive(Debug, Serialize)]
pub struct ToolError {
  pub code: String,
  pub message: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hint: Option<String>,
}

// ---------------------------------------------------------------------------
// Local state responses
// ---------------------------------------------------------------------------

/// Response for `get_current_branch` and `get_branch_metadata`.
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
pub struct BranchTreeResponse {
  pub root: String,
  pub tree_text: String,
  pub branches: Vec<BranchTreeNode>,
}

#[derive(Debug, Serialize)]
pub struct BranchTreeNode {
  pub branch: String,
  pub children: Vec<BranchTreeNode>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub jira_issue: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pr_number: Option<u32>,
}

/// Response for `get_branch_stack`.
#[derive(Debug, Serialize)]
pub struct BranchStackResponse {
  /// Ordered from the queried branch (index 0) up to the root.
  pub stack: Vec<BranchMetadataResponse>,
}

/// Response for `list_branches`.
#[derive(Debug, Serialize)]
pub struct ListBranchesResponse {
  pub branches: Vec<BranchMetadataResponse>,
}

/// Response for `list_repositories`.
#[derive(Debug, Serialize)]
pub struct ListRepositoriesResponse {
  pub repositories: Vec<RepositoryInfo>,
}

#[derive(Debug, Serialize)]
pub struct RepositoryInfo {
  pub name: String,
  pub path: String,
}

/// Response for `get_worktrees`.
#[derive(Debug, Serialize)]
pub struct ListWorktreesResponse {
  pub worktrees: Vec<WorktreeInfo>,
}

#[derive(Debug, Serialize)]
pub struct WorktreeInfo {
  pub name: String,
  pub path: String,
  pub branch: String,
}

// ---------------------------------------------------------------------------
// GitHub responses
// ---------------------------------------------------------------------------

/// Response for `get_pull_request`.
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
pub struct PrStatusResponse {
  pub pull_request: PullRequestResponse,
  pub reviews: Vec<ReviewInfo>,
  pub checks: Vec<CheckRunInfo>,
}

#[derive(Debug, Serialize)]
pub struct ReviewInfo {
  pub author: String,
  pub state: String,
}

#[derive(Debug, Serialize)]
pub struct CheckRunInfo {
  pub name: String,
  pub status: String,
  pub conclusion: Option<String>,
}

/// Response for `list_pull_requests`.
#[derive(Debug, Serialize)]
pub struct ListPullRequestsResponse {
  pub pull_requests: Vec<PullRequestResponse>,
}

// ---------------------------------------------------------------------------
// Jira responses
// ---------------------------------------------------------------------------

/// Response for `get_jira_issue`.
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
pub struct ListJiraIssuesResponse {
  pub issues: Vec<JiraIssueResponse>,
}
