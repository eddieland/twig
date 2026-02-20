//! Parameter structs for Jira tools.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetJiraIssueParams {
  /// Jira issue key (e.g. "PROJ-123"). Defaults to the current branch's issue if omitted.
  pub issue_key: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListJiraIssuesParams {
  /// Jira project key (e.g. "PROJ").
  pub project: String,
  /// Filter by status name (e.g. "In Progress").
  pub status: Option<String>,
  /// Filter by assignee (use "me" for the current user).
  pub assignee: Option<String>,
}
