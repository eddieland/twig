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

/// Represents a GitHub pull request
#[derive(Debug, Deserialize)]
pub struct GitHubPullRequest {
  pub number: u32,
  pub title: String,
  pub html_url: String,
  pub state: String,
  #[allow(dead_code)]
  pub user: GitHubUser,
  pub created_at: String,
  pub updated_at: String,
  pub head: GitHubPRRef,
  #[allow(dead_code)]
  pub base: GitHubPRRef,
  pub mergeable: Option<bool>,
  pub mergeable_state: Option<String>,
  pub draft: Option<bool>,
}

/// Represents a GitHub pull request reference (head or base)
#[derive(Debug, Deserialize)]
pub struct GitHubPRRef {
  #[allow(dead_code)]
  pub label: String,
  #[allow(dead_code)]
  pub ref_name: Option<String>,
  pub sha: String,
}

/// Represents a GitHub pull request review
#[derive(Debug, Deserialize)]
pub struct GitHubPRReview {
  #[allow(dead_code)]
  pub id: u64,
  pub user: GitHubUser,
  pub state: String,
  pub submitted_at: String,
}

/// Represents a GitHub check run
#[derive(Debug, Deserialize)]
pub struct GitHubCheckRun {
  #[allow(dead_code)]
  pub id: u64,
  pub name: String,
  pub status: String,
  pub conclusion: Option<String>,
  #[allow(dead_code)]
  pub started_at: String,
  #[allow(dead_code)]
  pub completed_at: Option<String>,
}

/// Represents a GitHub check suite
#[derive(Debug, Deserialize)]
pub struct GitHubCheckSuite {
  #[allow(dead_code)]
  pub id: u64,
  #[allow(dead_code)]
  pub status: String,
  #[allow(dead_code)]
  pub conclusion: Option<String>,
  #[allow(dead_code)]
  pub check_runs: Vec<GitHubCheckRun>,
}

/// Represents a GitHub PR status summary
#[derive(Debug)]
pub struct GitHubPRStatus {
  pub pr: GitHubPullRequest,
  pub reviews: Vec<GitHubPRReview>,
  pub check_runs: Vec<GitHubCheckRun>,
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

  /// Extract owner and repo from a GitHub URL
  pub fn extract_repo_info_from_url(&self, url: &str) -> Result<(String, String)> {
    // Match patterns like:
    // https://github.com/owner/repo
    // https://github.com/owner/repo.git
    // https://github.com/owner/repo/pull/123
    let re = regex::Regex::new(r"github\.com[/:]([^/]+)/([^/\.]+)").context("Failed to compile regex")?;

    if let Some(captures) = re.captures(url) {
      let owner = captures.get(1).unwrap().as_str().to_string();
      let repo = captures.get(2).unwrap().as_str().to_string();
      Ok((owner, repo))
    } else {
      Err(anyhow::anyhow!("Could not extract owner and repo from URL: {}", url))
    }
  }

  /// Extract PR number from a GitHub PR URL
  pub fn extract_pr_number_from_url(&self, url: &str) -> Result<u32> {
    // Match patterns like:
    // https://github.com/owner/repo/pull/123
    let re = regex::Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").context("Failed to compile regex")?;

    if let Some(captures) = re.captures(url) {
      let pr_str = captures.get(1).unwrap().as_str();
      let pr_number = pr_str
        .parse::<u32>()
        .with_context(|| format!("Failed to parse PR number '{pr_str}' as a valid integer"))?;
      Ok(pr_number)
    } else {
      Err(anyhow::anyhow!("Could not extract PR number from URL: {}", url))
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
