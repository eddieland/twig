mod client;
mod endpoints;
mod models;

// Re-export the client
pub use client::{JiraClient, create_jira_client};
// Re-export models
pub use models::{
  JiraAuth, JiraIssue, JiraIssueFields, JiraIssueStatus, JiraTransition, JiraTransitions, TransitionId,
  TransitionRequest,
};
