use anyhow::Result;
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
}

/// Create a Jira client from credentials
pub fn create_jira_client(base_url: &str, username: &str, api_token: &str) -> Result<JiraClient> {
  let auth = JiraAuth {
    username: username.to_string(),
    api_token: api_token.to_string(),
  };

  Ok(JiraClient::new(base_url, auth))
}
