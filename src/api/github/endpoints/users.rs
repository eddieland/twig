use anyhow::{Context, Result};
use reqwest::StatusCode;

use crate::api::github::client::GitHubClient;
use crate::api::github::models::GitHubUser;

impl GitHubClient {
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
        // First get the response body as text
        let body = response.text().await.context("Failed to read response body")?;

        // Then try to parse it as JSON
        let user = match serde_json::from_str::<GitHubUser>(&body) {
          Ok(user) => user,
          Err(e) => {
            // Try to extract the error message from the response
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body) {
              if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(anyhow::anyhow!(
                  "Failed to parse GitHub user: GitHub API error: {}",
                  message
                ));
              }
            }
            // Fall back to the original error if we can't extract a message
            return Err(anyhow::anyhow!("Failed to parse GitHub user: {}", e));
          }
        };

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
