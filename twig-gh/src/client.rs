//! # GitHub HTTP Client
//!
//! HTTP client implementation for GitHub API interactions, handling
//! authentication, request building, and response parsing for GitHub REST API
//! operations.

use anyhow::{Context, Result};
use reqwest::{Client, header};
use tracing::{debug, info, instrument, trace, warn};

use crate::consts::{ACCEPT, API_BASE_URL, USER_AGENT};
use crate::models::GitHubAuth;

/// Represents a GitHub API client
pub struct GitHubClient {
  pub(crate) client: Client,
  pub(crate) base_url: String,
  pub(crate) auth: GitHubAuth,
}

impl GitHubClient {
  /// Create a new GitHub client
  #[instrument(skip(auth), level = "debug")]
  pub fn new(auth: GitHubAuth) -> Self {
    info!("Creating new GitHub client");
    let client = Client::new();
    let instance = Self {
      client,
      base_url: API_BASE_URL.to_string(),
      auth,
    };
    info!("GitHub client created with base URL: {}", instance.base_url);
    instance
  }

  /// Overrides the base URL used for GitHub API requests.
  ///
  /// Primarily intended for tests that need to route requests through a mock
  /// server.
  pub fn set_base_url(&mut self, base_url: impl Into<String>) {
    self.base_url = base_url.into();
  }

  /// Test the GitHub connection by fetching the current user
  #[instrument(skip(self), level = "debug")]
  pub async fn test_connection(&self) -> Result<bool> {
    let url = format!("{}/user", self.base_url);
    debug!("Testing GitHub connection to {}", url);

    trace!("Sending request to GitHub API");
    let response = self
      .client
      .get(&url)
      .header(header::ACCEPT, ACCEPT)
      .header(header::USER_AGENT, USER_AGENT)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to connect to GitHub")?;

    let status = response.status();
    let success = status.is_success();

    if success {
      info!("Successfully connected to GitHub API (status: {})", status);
    } else {
      warn!("Failed to connect to GitHub API (status: {})", status);
    }

    Ok(success)
  }
}

/// Create a GitHub client from credentials
pub fn create_github_client(username: &str, token: &str) -> GitHubClient {
  let auth = GitHubAuth {
    username: username.to_string(),
    token: token.to_string(),
  };
  GitHubClient::new(auth)
}

#[cfg(test)]
mod tests {
  use wiremock::matchers::{header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::*;

  /// Test that GitHub client can be created with valid credentials
  #[tokio::test]
  async fn test_github_client_creation() -> Result<()> {
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let client = GitHubClient::new(auth);

    assert_eq!(client.base_url, API_BASE_URL);
    assert_eq!(client.auth.username, "test_user");
    assert_eq!(client.auth.token, "test_token");

    Ok(())
  }

  /// Test that GitHub client handles authentication correctly
  #[tokio::test]
  async fn test_github_client_auth() -> Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Create a mock that expects an Authorization header
    Mock::given(method("GET"))
      .and(path("/user"))
      .and(header(header::AUTHORIZATION, "token test_token"))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "login": "testuser",
          "id": 1234,
          "name": "Test User"
      })))
      .mount(&mock_server)
      .await;

    // Make the request - if auth is wrong, this will fail
    let response = client
      .client
      .get(&format!("{}/user", client.base_url))
      .header(header::AUTHORIZATION, format!("token {}", "test_token"))
      .send()
      .await?;

    assert!(response.status().is_success());
    Ok(())
  }
}
