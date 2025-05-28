//! # GitHub Pull Request Endpoints
//!
//! GitHub API endpoint implementations for pull request operations,
//! including fetching PRs, reviews, and status information.

use anyhow::{Context, Result};
use reqwest::StatusCode;

use crate::client::GitHubClient;
use crate::models::{GitHubPRReview, GitHubPRStatus, GitHubPullRequest};

impl GitHubClient {
  /// Get pull requests for a repository
  #[allow(dead_code)]
  pub async fn get_pull_requests(
    &self,
    owner: &str,
    repo: &str,
    state: Option<&str>,
  ) -> Result<Vec<GitHubPullRequest>> {
    let state_param = state.unwrap_or("open");
    let url = format!("{}/repos/{}/{}/pulls?state={}", self.base_url, owner, repo, state_param);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to fetch pull requests")?;

    match response.status() {
      StatusCode::OK => {
        // First get the response body as text
        let body = response.text().await.context("Failed to read response body")?;

        // Then try to parse it as JSON
        let prs = match serde_json::from_str::<Vec<GitHubPullRequest>>(&body) {
          Ok(prs) => prs,
          Err(e) => {
            // Try to extract the error message from the response
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body) {
              if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(anyhow::anyhow!(
                  "Failed to parse pull requests: GitHub API error: {}",
                  message
                ));
              }
            }
            // Fall back to the original error if we can't extract a message
            return Err(anyhow::anyhow!("Failed to parse pull requests: {}", e));
          }
        };

        Ok(prs)
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

  /// Get a specific pull request
  pub async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u32) -> Result<GitHubPullRequest> {
    let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to fetch pull request")?;

    match response.status() {
      StatusCode::OK => {
        // First get the response body as text
        let body = response.text().await.context("Failed to read response body")?;

        // Then try to parse it as JSON
        let pr = match serde_json::from_str::<GitHubPullRequest>(&body) {
          Ok(pr) => pr,
          Err(e) => {
            // Try to extract the error message from the response
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body) {
              if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(anyhow::anyhow!(
                  "Failed to parse pull request: GitHub API error: {}",
                  message
                ));
              }
            }
            // Fall back to the original error if we can't extract a message
            return Err(anyhow::anyhow!("Failed to parse pull request: {}", e));
          }
        };

        Ok(pr)
      }
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Pull request #{} not found", pr_number)),
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

  /// Get pull request reviews
  pub async fn get_pull_request_reviews(&self, owner: &str, repo: &str, pr_number: u32) -> Result<Vec<GitHubPRReview>> {
    let url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to fetch pull request reviews")?;

    match response.status() {
      StatusCode::OK => {
        // First get the response body as text
        let body = response.text().await.context("Failed to read response body")?;

        // Then try to parse it as JSON
        let reviews = match serde_json::from_str::<Vec<GitHubPRReview>>(&body) {
          Ok(reviews) => reviews,
          Err(e) => {
            // Try to extract the error message from the response
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body) {
              if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(anyhow::anyhow!(
                  "Failed to parse pull request reviews: GitHub API error: {}",
                  message
                ));
              }
            }
            // Fall back to the original error if we can't extract a message
            return Err(anyhow::anyhow!("Failed to parse pull request reviews: {}", e));
          }
        };

        Ok(reviews)
      }
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Pull request #{} not found", pr_number)),
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

  /// Find pull requests for a specific head branch
  pub async fn find_pull_requests_by_head_branch(
    &self,
    owner: &str,
    repo: &str,
    head_branch: &str,
  ) -> Result<Vec<GitHubPullRequest>> {
    // GitHub API supports filtering PRs by head branch using the format
    // "owner:branch" For local branches, we need to format it as
    // "owner:branch_name"
    let head_param = format!("{owner}:{head_branch}",);
    let url = format!("{}/repos/{owner}/{repo}/pulls", self.base_url);

    let response = self
      .client
      .get(&url)
      .query(&[("head", head_param.as_str()), ("state", "all")])
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to fetch pull requests by head branch")?;

    match response.status() {
      StatusCode::OK => {
        // First get the response body as text
        let body = response.text().await.context("Failed to read response body")?;

        // Then try to parse it as JSON
        let prs = match serde_json::from_str::<Vec<GitHubPullRequest>>(&body) {
          Ok(prs) => prs,
          Err(e) => {
            // Try to extract the error message from the response
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&body) {
              if let Some(message) = error_json.get("message").and_then(|m| m.as_str()) {
                return Err(anyhow::anyhow!(
                  "Failed to parse pull requests: GitHub API error: {}",
                  message
                ));
              }
            }
            // Fall back to the original error if we can't extract a message
            return Err(anyhow::anyhow!("Failed to parse pull requests: {}", e));
          }
        };

        Ok(prs)
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

  /// Get full PR status (PR details, reviews, and check runs)
  pub async fn get_pr_status(&self, owner: &str, repo: &str, pr_number: u32) -> Result<GitHubPRStatus> {
    // Get the PR details
    let pr = self.get_pull_request(owner, repo, pr_number).await?;

    // Get the reviews
    let reviews = self.get_pull_request_reviews(owner, repo, pr_number).await?;

    // Get the check runs
    let check_runs = self.get_check_runs(owner, repo, &pr.head.sha).await?;

    Ok(GitHubPRStatus {
      pr,
      reviews,
      check_runs,
    })
  }
}
#[cfg(test)]
mod tests {
  use wiremock::matchers::{header, method, path, query_param};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::client::GitHubClient;
  use crate::models::GitHubAuth;

  #[tokio::test]
  async fn test_get_pull_requests() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for open PRs
    Mock::given(method("GET"))
      .and(path("/repos/owner/repo/pulls"))
      .and(query_param("state", "open"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {
              "number": 1,
              "title": "Test PR",
              "html_url": "https://github.com/owner/repo/pull/1",
              "state": "open",
              "user": {
                  "login": "test_user",
                  "id": 1,
                  "name": "Test User"
              },
              "created_at": "2023-01-01T00:00:00Z",
              "updated_at": "2023-01-01T00:00:00Z",
              "head": {
                  "label": "owner:feature",
                  "ref_name": "feature",
                  "sha": "abc123"
              },
              "base": {
                  "label": "owner:main",
                  "ref_name": "main",
                  "sha": "def456"
              }
          }
      ])))
      .mount(&mock_server)
      .await;

    let prs = client.get_pull_requests("owner", "repo", Some("open")).await?;
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 1);
    assert_eq!(prs[0].title, "Test PR");
    assert_eq!(prs[0].state, "open");

    Ok(())
  }

  #[tokio::test]
  async fn test_get_pull_request() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for specific PR
    Mock::given(method("GET"))
      .and(path("/repos/owner/repo/pulls/1"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "number": 1,
          "title": "Test PR",
          "html_url": "https://github.com/owner/repo/pull/1",
          "state": "open",
          "user": {
              "login": "test_user",
              "id": 1,
              "name": "Test User"
          },
          "created_at": "2023-01-01T00:00:00Z",
          "updated_at": "2023-01-01T00:00:00Z",
          "head": {
              "label": "owner:feature",
              "ref_name": "feature",
              "sha": "abc123"
          },
          "base": {
              "label": "owner:main",
              "ref_name": "main",
              "sha": "def456"
          }
      })))
      .mount(&mock_server)
      .await;

    let pr = client.get_pull_request("owner", "repo", 1).await?;
    assert_eq!(pr.number, 1);
    assert_eq!(pr.title, "Test PR");
    assert_eq!(pr.state, "open");
    assert_eq!(pr.head.sha, "abc123");
    assert_eq!(pr.base.sha, "def456");

    Ok(())
  }

  #[tokio::test]
  async fn test_get_pull_request_reviews() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for PR reviews
    Mock::given(method("GET"))
      .and(path("/repos/owner/repo/pulls/1/reviews"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
          {
              "id": 1,
              "user": {
                  "login": "reviewer",
                  "id": 2,
                  "name": "Reviewer"
              },
              "state": "APPROVED",
              "submitted_at": "2023-01-01T00:00:00Z",
              "commit_id": "abc123",
              "body": "LGTM"
          }
      ])))
      .mount(&mock_server)
      .await;

    let reviews = client.get_pull_request_reviews("owner", "repo", 1).await?;
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].state, "APPROVED");
    assert_eq!(reviews[0].user.login, "reviewer");

    Ok(())
  }
}
