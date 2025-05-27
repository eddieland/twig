use anyhow::{Context, Result};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::client::GitHubClient;
use crate::models::GitHubCheckRun;

impl GitHubClient {
  /// Get check runs for a pull request
  pub async fn get_check_runs(&self, owner: &str, repo: &str, ref_sha: &str) -> Result<Vec<GitHubCheckRun>> {
    let url = format!(
      "{}/repos/{}/{}/commits/{}/check-runs",
      self.base_url, owner, repo, ref_sha
    );

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to fetch check runs")?;

    #[derive(Deserialize)]
    struct CheckRunsResponse {
      check_runs: Vec<GitHubCheckRun>,
    }

    match response.status() {
      StatusCode::OK => {
        // First get the response body as text
        let body = response.text().await.context("Failed to read response body")?;

        // Then try to parse it as JSON
        let check_runs_response = match serde_json::from_str::<CheckRunsResponse>(&body) {
          Ok(response) => response,
          Err(e) => {
            // Try to extract the error message from the response
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body) {
              if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(anyhow::anyhow!(
                  "Failed to parse check runs: GitHub API error: {}",
                  message
                ));
              }
            }
            // Fall back to the original error if we can't extract a message
            return Err(anyhow::anyhow!("Failed to parse check runs: {}", e));
          }
        };

        Ok(check_runs_response.check_runs)
      }
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Commit {} not found", ref_sha)),
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
