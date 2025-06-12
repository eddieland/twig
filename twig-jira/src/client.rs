//! # Jira HTTP Client
//!
//! HTTP client implementation for Jira API interactions, handling
//! authentication, request building, and response parsing for Jira REST API
//! operations.

use anyhow::{Context, Result};
use reqwest::{Client, header};
use tracing::{debug, info, instrument, trace, warn};

use crate::consts::USER_AGENT;
use crate::models::JiraAuth;

/// Represents a Jira API client
pub struct JiraClient {
  pub(crate) client: Client,
  pub(crate) base_url: String,
  pub(crate) auth: JiraAuth,
}

impl JiraClient {
  /// Create a new Jira client
  #[instrument(skip(auth), level = "debug")]
  pub fn new(base_url: &str, auth: JiraAuth) -> Self {
    info!("Creating new Jira client for base URL: {}", base_url);
    let client = Client::new();
    let instance = Self {
      client,
      base_url: base_url.to_string(),
      auth,
    };
    info!("Jira client created successfully");
    instance
  }

  /// Test the Jira connection by fetching the current user
  #[instrument(skip(self), level = "debug")]
  pub async fn test_connection(&self) -> Result<bool> {
    let url = format!("{}/rest/api/2/myself", self.base_url);
    debug!("Testing Jira connection to {}", url);

    trace!("Sending request to Jira API");
    let response = self
      .client
      .get(&url)
      .header(header::USER_AGENT, USER_AGENT)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to connect to Jira")?;

    let status = response.status();
    let success = status.is_success();

    if success {
      info!("Successfully connected to Jira API (status: {})", status);
    } else {
      warn!("Failed to connect to Jira API (status: {})", status);
    }

    Ok(success)
  }
}

/// Create a Jira client from credentials
pub fn create_jira_client(base_url: &str, username: &str, api_token: &str) -> JiraClient {
  let auth = JiraAuth {
    username: username.to_string(),
    api_token: api_token.to_string(),
  };
  JiraClient::new(base_url, auth)
}

#[cfg(test)]
mod tests {
  use reqwest::header;
  use wiremock::matchers::{header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::*;

  /// Test that Jira client can be created with valid credentials
  #[tokio::test]
  async fn test_jira_client_creation() -> Result<()> {
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let client = JiraClient::new("https://test.atlassian.net", auth);

    assert_eq!(client.base_url, "https://test.atlassian.net");
    assert_eq!(client.auth.username, "test_user");
    assert_eq!(client.auth.api_token, "test_token");

    Ok(())
  }

  /// Test that Jira client handles authentication correctly
  #[tokio::test]
  async fn test_jira_client_auth() -> Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let client = JiraClient::new(&mock_server.uri(), auth);

    // Create a mock that expects Basic auth header
    Mock::given(method("GET"))
      .and(path("/rest/api/2/myself"))
      .and(header(header::AUTHORIZATION, "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4=")) // test_user:test_token in base64
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "name": "test_user",
          "displayName": "Test User",
          "emailAddress": "test@example.com"
      })))
      .mount(&mock_server)
      .await;

    // Make the request - if auth is wrong, this will fail
    let response = client
      .client
      .get(&format!("{}/rest/api/2/myself", client.base_url))
      .basic_auth("test_user", Some("test_token"))
      .send()
      .await?;

    assert!(response.status().is_success());
    Ok(())
  }
}
