use anyhow::{Context, Result};
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::instrument;

use crate::client::GitHubClient;
use crate::models::CheckRun;

impl GitHubClient {
  /// Get check runs for a pull request
  #[instrument(skip(self), level = "debug")]
  pub async fn get_check_runs(&self, owner: &str, repo: &str, ref_sha: &str) -> Result<Vec<CheckRun>> {
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
      check_runs: Vec<CheckRun>,
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
#[cfg(test)]
mod tests {
  use wiremock::matchers::{header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::client::GitHubClient;
  use crate::models::GitHubAuth;

  #[tokio::test]
  async fn test_get_check_runs() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    let ref_sha = "abc123def456";

    // Mock response for check runs
    Mock::given(method("GET"))
      .and(path(format!("/repos/owner/repo/commits/{}/check-runs", ref_sha)))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "total_count": 2,
          "check_runs": [
              {
                  "id": 1,
                  "name": "test-suite",
                  "status": "completed",
                  "conclusion": "success",
                  "started_at": "2023-01-01T00:00:00Z",
                  "completed_at": "2023-01-01T00:01:00Z"
              },
              {
                  "id": 2,
                  "name": "lint",
                  "status": "completed",
                  "conclusion": "failure",
                  "started_at": "2023-01-01T00:00:00Z",
                  "completed_at": "2023-01-01T00:01:00Z"
              }
          ]
      })))
      .mount(&mock_server)
      .await;

    let check_runs = client.get_check_runs("owner", "repo", ref_sha).await?;
    assert_eq!(check_runs.len(), 2);

    // Verify first check run
    assert_eq!(check_runs[0].id, 1);
    assert_eq!(check_runs[0].name, "test-suite");
    assert_eq!(check_runs[0].status, "completed");
    assert_eq!(check_runs[0].conclusion, Some("success".to_string()));

    // Verify second check run
    assert_eq!(check_runs[1].id, 2);
    assert_eq!(check_runs[1].name, "lint");
    assert_eq!(check_runs[1].status, "completed");
    assert_eq!(check_runs[1].conclusion, Some("failure".to_string()));

    Ok(())
  }

  #[tokio::test]
  async fn test_get_check_runs_not_found() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    let ref_sha = "nonexistent";

    // Mock 404 response
    Mock::given(method("GET"))
      .and(path(format!("/repos/owner/repo/commits/{}/check-runs", ref_sha)))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
          "message": "Not Found",
          "documentation_url": "https://docs.github.com/v3/checks/runs/#list-check-runs-for-a-git-reference"
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_check_runs("owner", "repo", ref_sha).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    Ok(())
  }
}
