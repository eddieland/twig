use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// Represents a Jira API client
pub struct JiraClient {
  client: Client,
  base_url: String,
  auth: JiraAuth,
}

/// Represents Jira authentication credentials
#[derive(Clone)]
pub struct JiraAuth {
  username: String,
  api_token: String,
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
  pub name: String,
}

impl JiraClient {
  /// Create a new Jira client
  pub fn new(base_url: &str, auth: JiraAuth) -> Self {
    let client = Client::new();
    Self {
      client,
      base_url: base_url.to_string(),
      auth,
    }
  }

  /// Test the Jira connection by fetching the current user
  #[allow(dead_code)]
  pub async fn test_connection(&self) -> Result<bool> {
    let url = format!("{}/rest/api/2/myself", self.base_url);

    let response = self
      .client
      .get(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to connect to Jira")?;

    Ok(response.status().is_success())
  }

  /// Get a Jira issue by key
  pub async fn get_issue(&self, issue_key: &str) -> Result<JiraIssue> {
    let url = format!("{}/rest/api/2/issue/{}", self.base_url, issue_key);

    let response = self
      .client
      .get(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to fetch Jira issue")?;

    match response.status() {
      StatusCode::OK => {
        let issue = response
          .json::<JiraIssue>()
          .await
          .context("Failed to parse Jira issue")?;
        Ok(issue)
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
}

/// Create a Jira client from credentials
pub fn create_jira_client(base_url: &str, username: &str, api_token: &str) -> Result<JiraClient> {
  let auth = JiraAuth {
    username: username.to_string(),
    api_token: api_token.to_string(),
  };

  Ok(JiraClient::new(base_url, auth))
}
