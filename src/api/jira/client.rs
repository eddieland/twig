use anyhow::{Context, Result};
use reqwest::Client;

use crate::api::jira::models::JiraAuth;

/// Represents a Jira API client
pub struct JiraClient {
  pub(crate) client: Client,
  pub(crate) base_url: String,
  pub(crate) auth: JiraAuth,
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
}

/// Create a Jira client from credentials
pub fn create_jira_client(base_url: &str, username: &str, api_token: &str) -> Result<JiraClient> {
  let auth = JiraAuth {
    username: username.to_string(),
    api_token: api_token.to_string(),
  };

  Ok(JiraClient::new(base_url, auth))
}
