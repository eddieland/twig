use anyhow::{Context, Result};
use reqwest::StatusCode;

use crate::client::JiraClient;
use crate::models::{JiraTransition, JiraTransitions, TransitionId, TransitionRequest};

impl JiraClient {
  /// Get available transitions for an issue
  pub async fn get_transitions(&self, issue_key: &str) -> Result<Vec<JiraTransition>> {
    let url = format!("{}/rest/api/2/issue/{}/transitions", self.base_url, issue_key);

    let response = self
      .client
      .get(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to fetch Jira transitions")?;

    match response.status() {
      StatusCode::OK => {
        let transitions = response
          .json::<JiraTransitions>()
          .await
          .context("Failed to parse Jira transitions")?;
        Ok(transitions.transitions)
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(anyhow::anyhow!(
        "Authentication failed. Please check your Jira credentials."
      )),
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Issue {} not found", issue_key)),
      _ => Err(anyhow::anyhow!(
        "Unexpected error: HTTP {} - {}",
        response.status(),
        response.text().await.unwrap_or_default()
      )),
    }
  }

  /// Transition an issue to a new status
  pub async fn transition_issue(&self, issue_key: &str, transition_id: &str) -> Result<()> {
    let url = format!("{}/rest/api/2/issue/{}/transitions", self.base_url, issue_key);

    let payload = TransitionRequest {
      transition: TransitionId {
        id: transition_id.to_string(),
      },
    };

    let response = self
      .client
      .post(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .json(&payload)
      .send()
      .await
      .context("Failed to transition Jira issue")?;

    match response.status() {
      StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(anyhow::anyhow!(
        "Authentication failed. Please check your Jira credentials."
      )),
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Issue {} not found", issue_key)),
      StatusCode::BAD_REQUEST => Err(anyhow::anyhow!(
        "Invalid transition. The transition may not be available for the current status."
      )),
      _ => Err(anyhow::anyhow!(
        "Unexpected error: HTTP {} - {}",
        response.status(),
        response.text().await.unwrap_or_default()
      )),
    }
  }
}

#[cfg(test)]
mod tests {
  use wiremock::matchers::{basic_auth, body_json, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::client::JiraClient;
  use crate::models::JiraAuth;

  #[tokio::test]
  async fn test_get_transitions() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock response for transitions
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/TEST-123/transitions"))
      .and(basic_auth("test_user", "test_token"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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
      })))
      .mount(&mock_server)
      .await;

    let transitions = client.get_transitions("TEST-123").await?;
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].id, "11");
    assert_eq!(transitions[0].name, "To Do");
    assert_eq!(transitions[2].id, "31");
    assert_eq!(transitions[2].name, "Done");

    Ok(())
  }

  #[tokio::test]
  async fn test_transition_issue() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock response for transition
    Mock::given(method("POST"))
      .and(path("/rest/api/2/issue/TEST-123/transitions"))
      .and(basic_auth("test_user", "test_token"))
      .and(body_json(serde_json::json!({
          "transition": {
              "id": "21"
          }
      })))
      .respond_with(ResponseTemplate::new(204))
      .mount(&mock_server)
      .await;

    let result = client.transition_issue("TEST-123", "21").await;
    assert!(result.is_ok());

    Ok(())
  }

  #[tokio::test]
  async fn test_transition_issue_invalid_transition() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock response for invalid transition
    Mock::given(method("POST"))
      .and(path("/rest/api/2/issue/TEST-123/transitions"))
      .and(basic_auth("test_user", "test_token"))
      .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
          "errorMessages": ["The requested transition is not available for the current status."],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.transition_issue("TEST-123", "invalid").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid transition"));

    Ok(())
  }

  #[tokio::test]
  async fn test_transitions_not_found() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock 404 response
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/NONEXISTENT-123/transitions"))
      .and(basic_auth("test_user", "test_token"))
      .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
          "errorMessages": ["Issue does not exist or you do not have permission to see it."],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_transitions("NONEXISTENT-123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    Ok(())
  }

  #[tokio::test]
  async fn test_transitions_unauthorized() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "invalid_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock unauthorized response
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/TEST-123/transitions"))
      .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
          "errorMessages": ["Authentication failed"],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_transitions("TEST-123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Authentication failed"));

    Ok(())
  }
}
