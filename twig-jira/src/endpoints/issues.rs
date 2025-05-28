//! # Jira Issue Endpoints
//!
//! Jira API endpoint implementations for issue operations,
//! including fetching, creating, and updating Jira issues.

use anyhow::{Context, Result};
use reqwest::StatusCode;
use tracing::{debug, info, instrument, trace, warn};

use crate::client::JiraClient;
use crate::models::JiraIssue;

impl JiraClient {
  /// Get a Jira issue by key
  #[instrument(skip(self), level = "debug")]
  pub async fn get_issue(&self, issue_key: &str) -> Result<JiraIssue> {
    let url = format!("{}/rest/api/2/issue/{}", self.base_url, issue_key);
    debug!("Fetching Jira issue: {}", issue_key);
    trace!("Jira API URL: {}", url);

    let response = self
      .client
      .get(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to fetch Jira issue")?;

    let status = response.status();
    debug!("Jira API response status: {}", status);

    match status {
      StatusCode::OK => {
        debug!("Successfully received Jira issue data");
        let issue = response
          .json::<JiraIssue>()
          .await
          .context("Failed to parse Jira issue")?;

        info!("Successfully fetched Jira issue: {}", issue_key);
        trace!("Issue summary: {}", issue.fields.summary);

        Ok(issue)
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing Jira API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your Jira credentials."
        ))
      }
      StatusCode::NOT_FOUND => {
        warn!("Jira issue not found: {}", issue_key);
        Err(anyhow::anyhow!("Issue {} not found", issue_key))
      }
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected Jira API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {} - {}", status, error_text))
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use wiremock::matchers::{basic_auth, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::client::JiraClient;
  use crate::models::JiraAuth;

  #[tokio::test]
  async fn test_get_issue() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock response for issue
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/TEST-123"))
      .and(basic_auth("test_user", "test_token"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "id": "10000",
          "key": "TEST-123",
          "fields": {
              "summary": "Test issue",
              "description": "This is a test issue",
              "status": {
                  "id": "10001",
                  "name": "In Progress",
                  "statusCategory": {
                      "id": 4,
                      "key": "indeterminate",
                      "name": "In Progress"
                  }
              }
          }
      })))
      .mount(&mock_server)
      .await;

    let issue = client.get_issue("TEST-123").await?;
    assert_eq!(issue.key, "TEST-123");
    assert_eq!(issue.fields.summary, "Test issue");
    assert_eq!(issue.fields.status.name, "In Progress");

    Ok(())
  }

  #[tokio::test]
  async fn test_get_issue_not_found() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock 404 response
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/NONEXISTENT-123"))
      .and(basic_auth("test_user", "test_token"))
      .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
          "errorMessages": ["Issue does not exist or you do not have permission to see it."],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_issue("NONEXISTENT-123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    Ok(())
  }

  #[tokio::test]
  async fn test_get_issue_unauthorized() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "invalid_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock unauthorized response
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/TEST-123"))
      .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
          "errorMessages": ["Authentication failed"],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_issue("TEST-123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Authentication failed"));

    Ok(())
  }
}
