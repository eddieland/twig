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

    let issue: JiraIssue = serde_json::from_value(json).unwrap();

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

    let transitions: JiraTransitions = serde_json::from_value(json).unwrap();

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
