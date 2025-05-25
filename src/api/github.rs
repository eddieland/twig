use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// Represents a GitHub API client
pub struct GitHubClient {
  client: Client,
  base_url: String,
  auth: GitHubAuth,
}

/// Represents GitHub authentication credentials
#[derive(Clone)]
pub struct GitHubAuth {
  username: String,
  token: String,
}

/// Represents a GitHub user
#[derive(Debug, Deserialize)]
pub struct GitHubUser {
  pub login: String,
  pub id: u64,
  pub name: Option<String>,
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

  /// Get the current authenticated user
  pub async fn get_current_user(&self) -> Result<GitHubUser> {
    let url = format!("{}/user", self.base_url);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to fetch GitHub user")?;

    match response.status() {
      StatusCode::OK => {
        let user = response
          .json::<GitHubUser>()
          .await
          .context("Failed to parse GitHub user")?;
        Ok(user)
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(anyhow::anyhow!(
        "Authentication failed. Please check your GitHub credentials."
      )),
      _ => Err(anyhow::anyhow!(
        "Unexpected error: HTTP {} - {}",
        response.status(),
        response.text().await.unwrap_or_default()
      )),
    }
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
