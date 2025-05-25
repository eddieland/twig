use serde::{Deserialize, Serialize};

/// Represents Jira authentication credentials
#[derive(Clone)]
pub struct JiraAuth {
  pub username: String,
  pub api_token: String,
}

/// Represents a Jira issue
#[derive(Debug, Deserialize)]
pub struct JiraIssue {
  #[allow(dead_code)]
  pub id: String,
  pub key: String,
  pub fields: JiraIssueFields,
}

/// Represents Jira issue fields
#[derive(Debug, Deserialize)]
pub struct JiraIssueFields {
  pub summary: String,
  pub description: Option<String>,
  pub status: JiraIssueStatus,
}

/// Represents a Jira issue status
#[derive(Debug, Deserialize)]
pub struct JiraIssueStatus {
  #[allow(dead_code)]
  pub id: Option<String>,
  pub name: String,
}

/// Represents a Jira transition
#[derive(Debug, Deserialize)]
pub struct JiraTransition {
  pub id: String,
  pub name: String,
}

/// Represents a list of Jira transitions
#[derive(Debug, Deserialize)]
pub struct JiraTransitions {
  pub transitions: Vec<JiraTransition>,
}

/// Represents a transition request payload
#[derive(Debug, Serialize)]
pub struct TransitionRequest {
  pub transition: TransitionId,
}

/// Represents a transition ID for the request
#[derive(Debug, Serialize)]
pub struct TransitionId {
  pub id: String,
}
