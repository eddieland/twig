use anyhow::{Context, Result};
use reqwest::Client;

use crate::api::github::models::GitHubAuth;

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
