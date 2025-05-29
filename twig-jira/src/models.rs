//! # Jira API Models
//!
//! Data structures and serialization models for Jira API responses,
//! including issues, transitions, statuses, and authentication types.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Represents Jira authentication credentials
#[derive(Clone)]
pub struct JiraAuth {
  pub username: String,
  pub api_token: String,
}

/// Represents a Jira issue
#[derive(Debug, Deserialize, Serialize)]
pub struct Issue {
  pub id: String,
  pub key: String,
  pub fields: IssueFields,
}

/// Represents Jira issue fields
#[derive(Debug, Deserialize, Serialize)]
pub struct IssueFields {
  pub summary: String,
  pub description: Option<String>,
  pub status: IssueStatus,
  pub assignee: Option<JiraUser>,
  #[serde(default)]
  pub updated: String,
}

/// Represents a Jira user
#[derive(Debug, Deserialize, Serialize)]
pub struct JiraUser {
  pub name: String,
  #[serde(rename = "displayName")]
  pub display_name: String,
  #[serde(rename = "emailAddress", default)]
  pub email_address: Option<String>,
}

/// Represents a Jira issue status
#[derive(Debug, Deserialize, Serialize)]
pub struct IssueStatus {
  pub id: Option<String>,
  pub name: String,
}

/// Represents a Jira transition
#[derive(Debug, Deserialize, Serialize)]
pub struct Transition {
  pub id: String,
  pub name: String,
}

/// Represents a list of Jira transitions
#[derive(Debug, Deserialize, Serialize)]
pub struct Transitions {
  pub transitions: Vec<Transition>,
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

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn test_jira_auth() {
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };

    assert_eq!(auth.username, "test_user");
    assert_eq!(auth.api_token, "test_token");
  }

  #[test]
  fn test_jira_issue_deserialization() {
    let json = json!({
        "id": "10000",
        "key": "PROJ-123",
        "fields": {
            "summary": "Test issue",
            "description": "This is a test issue",
            "status": {
                "name": "In Progress"
            }
        }
    });

    let issue: Issue = serde_json::from_value(json).unwrap();

    assert_eq!(issue.id, "10000");
    assert_eq!(issue.key, "PROJ-123");
    assert_eq!(issue.fields.summary, "Test issue");
    assert_eq!(issue.fields.description, Some("This is a test issue".to_string()));
    assert_eq!(issue.fields.status.name, "In Progress");
  }

  #[test]
  fn test_jira_transitions_deserialization() {
    let json = json!({
        "transitions": [
            {
                "id": "11",
                "name": "To Do"
            },
            {
                "id": "21",
                "name": "In Progress"
            },
            {
                "id": "31",
                "name": "Done"
            }
        ]
    });

    let transitions: Transitions = serde_json::from_value(json).unwrap();

    assert_eq!(transitions.transitions.len(), 3);
    assert_eq!(transitions.transitions[0].id, "11");
    assert_eq!(transitions.transitions[0].name, "To Do");
    assert_eq!(transitions.transitions[2].id, "31");
    assert_eq!(transitions.transitions[2].name, "Done");
  }

  #[test]
  fn test_jira_transition_request_serialization() {
    let request = TransitionRequest {
      transition: TransitionId { id: "21".to_string() },
    };

    let json = serde_json::to_value(&request).unwrap();

    assert_eq!(
      json,
      json!({
          "transition": {
              "id": "21"
          }
      })
    );
  }
}
