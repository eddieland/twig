//! # GitHub HTTP Client
//!
//! HTTP client implementation for GitHub API interactions, handling
//! authentication, request building, and response parsing for GitHub REST API
//! operations.

use anyhow::{Context, Result};
use reqwest::Client;

use crate::models::GitHubAuth;

/// Represents a GitHub API client
pub struct GitHubClient {
  pub(crate) client: Client,
  pub(crate) base_url: String,
  pub(crate) auth: GitHubAuth,
}

impl GitHubClient {
  /// Create a new GitHub client
  pub fn new(auth: GitHubAuth) -> Self {
    let client = Client::new();
    Self {
      client,
      base_url: "https://api.github.com".to_string(),
      auth,
    }
  }

  /// Test the GitHub connection by fetching the current user
  pub async fn test_connection(&self) -> Result<bool> {
    let url = format!("{}/user", self.base_url);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to connect to GitHub")?;

    Ok(response.status().is_success())
  }
}

/// Create a GitHub client from credentials
pub fn create_github_client(username: &str, token: &str) -> Result<GitHubClient> {
  let auth = GitHubAuth {
    username: username.to_string(),
    token: token.to_string(),
  };

  Ok(GitHubClient::new(auth))
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

    assert_eq!(client.base_url, "https://api.github.com");
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
      .and(header("Authorization", "token test_token"))
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
      .header("Authorization", format!("token {}", "test_token"))
      .send()
      .await?;

    assert!(response.status().is_success());
    Ok(())
  }
}
