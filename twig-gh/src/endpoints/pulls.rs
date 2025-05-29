use anyhow::{Context, Result};
use tracing::{debug, instrument, trace, warn};

use crate::client::GitHubClient;
use crate::models::{GitHubPRReview, GitHubPRStatus, GitHubPullRequest};

/// Pagination options for GitHub API requests
#[derive(Debug, Clone, Copy)]
pub struct PaginationOptions {
  /// Number of items per page
  pub per_page: u32,
  /// Page number (1-based)
  pub page: u32,
}

impl Default for PaginationOptions {
  fn default() -> Self {
    Self { per_page: 30, page: 1 }
  }
}

impl GitHubClient {
  /// List pull requests for a repository with pagination support
  #[instrument(skip(self), level = "debug")]
  pub async fn list_pull_requests(
    &self,
    owner: &str,
    repo: &str,
    state: Option<&str>,
    pagination_options: Option<PaginationOptions>,
  ) -> Result<Vec<GitHubPullRequest>> {
    // Set default state to "open" if not provided
    let state_param = state.unwrap_or("open");
    let pagination = pagination_options.unwrap_or_default();

    debug!(
      "Listing pull requests for {}/{} with state={}",
      owner, repo, state_param
    );

    let url = format!(
      "{}/repos/{}/{}/pulls?state={}&per_page={}&page={}",
      self.base_url, owner, repo, state_param, pagination.per_page, pagination.page
    );

    trace!("GitHub API URL: {}", url);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to send request to GitHub API")?;

    if !response.status().is_success() {
      let status = response.status();
      let error_text = response.text().await.unwrap_or_default();
      return Err(anyhow::anyhow!(
        "GitHub API returned error status {}: {}",
        status,
        error_text
      ));
    }

    let pull_requests: Vec<GitHubPullRequest> = response.json().await.context("Failed to parse GitHub API response")?;

    Ok(pull_requests)
  }

  /// Get pull requests for a repository (legacy method, use list_pull_requests
  /// instead)
  #[allow(dead_code)]
  pub async fn get_pull_requests(
    &self,
    owner: &str,
    repo: &str,
    state: Option<&str>,
  ) -> Result<Vec<GitHubPullRequest>> {
    self.list_pull_requests(owner, repo, state, None).await
  }
  /// Get a specific pull request by number
  #[instrument(skip(self), level = "debug")]
  pub async fn get_pull_request(&self, owner: &str, repo: &str, pr_number: u32) -> Result<GitHubPullRequest> {
    debug!("Fetching pull request #{} for {}/{}", pr_number, owner, repo);

    let url = format!("{}/repos/{}/{}/pulls/{}", self.base_url, owner, repo, pr_number);

    trace!("GitHub API URL: {}", url);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to send request to GitHub API")?;

    let status = response.status();
    debug!("GitHub API response status: {}", status);

    match status {
      reqwest::StatusCode::OK => {
        debug!("Successfully received pull request data");
        let pull_request = response
          .json::<GitHubPullRequest>()
          .await
          .context("Failed to parse GitHub pull request")?;

        trace!("Pull request title: {}", pull_request.title);
        Ok(pull_request)
      }
      reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing GitHub API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your GitHub credentials."
        ))
      }
      reqwest::StatusCode::NOT_FOUND => {
        warn!("Pull request not found: {}/{} #{}", owner, repo, pr_number);
        Err(anyhow::anyhow!("Pull request #{} not found", pr_number))
      }
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected GitHub API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {} - {}", status, error_text))
      }
    }
  }

  /// Get pull request reviews
  #[instrument(skip(self), level = "debug")]
  pub async fn get_pull_request_reviews(&self, owner: &str, repo: &str, pr_number: u32) -> Result<Vec<GitHubPRReview>> {
    debug!("Fetching reviews for PR #{} in {}/{}", pr_number, owner, repo);

    let url = format!("{}/repos/{}/{}/pulls/{}/reviews", self.base_url, owner, repo, pr_number);

    trace!("GitHub API URL: {}", url);

    let response = self
      .client
      .get(&url)
      .header("Accept", "application/vnd.github.v3+json")
      .header("User-Agent", "twig-cli")
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context("Failed to send request to GitHub API")?;

    let status = response.status();
    debug!("GitHub API response status: {}", status);

    match status {
      reqwest::StatusCode::OK => {
        debug!("Successfully received PR reviews data");
        let reviews = response
          .json::<Vec<GitHubPRReview>>()
          .await
          .context("Failed to parse GitHub PR reviews")?;

        trace!("Received {} reviews", reviews.len());
        Ok(reviews)
      }
      reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing GitHub API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your GitHub credentials."
        ))
      }
      reqwest::StatusCode::NOT_FOUND => {
        warn!("Pull request not found: {}/{} #{}", owner, repo, pr_number);
        Err(anyhow::anyhow!("Pull request #{} not found", pr_number))
      }
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected GitHub API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {} - {}", status, error_text))
      }
    }
  }

  /// Get comprehensive PR status including the PR details, reviews, and check
  /// runs
  #[instrument(skip(self), level = "debug")]
  pub async fn get_pr_status(&self, owner: &str, repo: &str, pr_number: u32) -> Result<GitHubPRStatus> {
    debug!("Fetching PR status for #{} in {}/{}", pr_number, owner, repo);

    // Get the PR details
    let pr = self.get_pull_request(owner, repo, pr_number).await?;

    // Get the PR reviews
    let reviews = self.get_pull_request_reviews(owner, repo, pr_number).await?;

    // Get the check runs for the PR's head commit
    let check_runs = self.get_check_runs(owner, repo, &pr.head.sha).await?;

    // Combine all the data into a GitHubPRStatus
    let status = GitHubPRStatus {
      pr,
      reviews,
      check_runs,
    };

    debug!(
      "Successfully fetched PR status with {} reviews and {} check runs",
      status.reviews.len(),
      status.check_runs.len()
    );

    Ok(status)
  }
  /// Find pull requests by head branch name
  #[instrument(skip(self), level = "debug")]
  pub async fn find_pull_requests_by_head_branch(
    &self,
    owner: &str,
    repo: &str,
    branch_name: &str,
    state: Option<&str>,
  ) -> Result<Vec<GitHubPullRequest>> {
    debug!(
      "Finding pull requests for {}/{} with head branch: {}",
      owner, repo, branch_name
    );

    // Get all pull requests for the repository
    let pull_requests = self.list_pull_requests(owner, repo, state, None).await?;

    // Filter pull requests by head branch name
    let matching_prs: Vec<GitHubPullRequest> = pull_requests
      .into_iter()
      .filter(|pr| {
        if let Some(ref_name) = &pr.head.ref_name {
          ref_name == branch_name
        } else {
          // If ref_name is None, check if the branch name is in the label
          // Label format is typically "username:branch-name"
          pr.head.label.split(':').nth(1) == Some(branch_name)
        }
      })
      .collect();

    debug!(
      "Found {} pull requests with head branch: {}",
      matching_prs.len(),
      branch_name
    );
    Ok(matching_prs)
  }
}

#[cfg(test)]
mod tests {
  use wiremock::matchers::{header, method, path, query_param};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::*;
  use crate::models::GitHubAuth;

  #[tokio::test]
  async fn test_list_pull_requests() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for pull requests
    Mock::given(method("GET"))
      .and(path("/repos/octocat/Hello-World/pulls"))
      .and(query_param("state", "open"))
      .and(query_param("per_page", "30"))
      .and(query_param("page", "1"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
          "id": 1,
          "number": 1347,
          "state": "open",
          "title": "Amazing new feature",
          "html_url": "https://github.com/octocat/Hello-World/pull/1347",
          "user": {
            "login": "octocat",
            "id": 1,
            "type": "User"
          },
          "created_at": "2011-01-26T19:01:12Z",
          "updated_at": "2011-01-26T19:01:12Z",
          "closed_at": null,
          "merged_at": null,
          "head": {
            "label": "octocat:new-feature",
            "ref_name": "new-feature",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          },
          "base": {
            "label": "octocat:master",
            "ref_name": "master",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          }
        }
      ])))
      .mount(&mock_server)
      .await;

    let prs = client
      .list_pull_requests("octocat", "Hello-World", Some("open"), None)
      .await?;

    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 1347);
    assert_eq!(prs[0].title, "Amazing new feature");
    assert_eq!(prs[0].state, "open");
    assert_eq!(prs[0].user.login, "octocat");

    Ok(())
  }

  #[tokio::test]
  async fn test_list_pull_requests_with_pagination() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for pull requests with pagination
    Mock::given(method("GET"))
      .and(path("/repos/octocat/Hello-World/pulls"))
      .and(query_param("state", "closed"))
      .and(query_param("per_page", "5"))
      .and(query_param("page", "2"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
          "id": 2,
          "number": 1348,
          "state": "closed",
          "title": "Another feature",
          "html_url": "https://github.com/octocat/Hello-World/pull/1348",
          "user": {
            "login": "octocat",
            "id": 1,
            "type": "User"
          },
          "created_at": "2011-01-26T19:01:12Z",
          "updated_at": "2011-01-26T19:01:12Z",
          "closed_at": "2011-01-27T19:01:12Z",
          "merged_at": "2011-01-27T19:01:12Z",
          "head": {
            "label": "octocat:another-feature",
            "ref_name": "another-feature",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          },
          "base": {
            "label": "octocat:master",
            "ref_name": "master",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          }
        }
      ])))
      .mount(&mock_server)
      .await;

    let pagination = PaginationOptions { per_page: 5, page: 2 };
    let prs = client
      .list_pull_requests("octocat", "Hello-World", Some("closed"), Some(pagination))
      .await?;

    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 1348);
    assert_eq!(prs[0].title, "Another feature");
    assert_eq!(prs[0].state, "closed");

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

    // Mock response for a specific pull request
    Mock::given(method("GET"))
      .and(path("/repos/octocat/Hello-World/pulls/1347"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "id": 1,
        "number": 1347,
        "state": "open",
        "title": "Amazing new feature",
        "html_url": "https://github.com/octocat/Hello-World/pull/1347",
        "user": {
          "login": "octocat",
          "id": 1,
          "name": "The Octocat"
        },
        "created_at": "2011-01-26T19:01:12Z",
        "updated_at": "2011-01-26T19:01:12Z",
        "head": {
          "label": "octocat:new-feature",
          "ref_name": "new-feature",
          "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
        },
        "base": {
          "label": "octocat:master",
          "ref_name": "master",
          "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
        },
        "mergeable": true,
        "mergeable_state": "clean",
        "draft": false
      })))
      .mount(&mock_server)
      .await;

    // Test getting a specific pull request
    let pr = client.get_pull_request("octocat", "Hello-World", 1347).await?;

    assert_eq!(pr.number, 1347);
    assert_eq!(pr.title, "Amazing new feature");
    assert_eq!(pr.state, "open");
    assert_eq!(pr.html_url, "https://github.com/octocat/Hello-World/pull/1347");
    assert_eq!(pr.user.login, "octocat");
    assert_eq!(pr.mergeable, Some(true));
    assert_eq!(pr.draft, Some(false));

    Ok(())
  }

  #[tokio::test]
  async fn test_find_pull_requests_by_head_branch() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for pull requests
    Mock::given(method("GET"))
      .and(path("/repos/octocat/Hello-World/pulls"))
      .and(query_param("state", "open"))
      .and(query_param("per_page", "30"))
      .and(query_param("page", "1"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
          "id": 1,
          "number": 1347,
          "state": "open",
          "title": "Feature from target-branch",
          "html_url": "https://github.com/octocat/Hello-World/pull/1347",
          "user": {
            "login": "octocat",
            "id": 1,
            "type": "User"
          },
          "created_at": "2011-01-26T19:01:12Z",
          "updated_at": "2011-01-26T19:01:12Z",
          "head": {
            "label": "octocat:target-branch",
            "ref_name": "target-branch",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          },
          "base": {
            "label": "octocat:master",
            "ref_name": "master",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          }
        },
        {
          "id": 2,
          "number": 1348,
          "state": "open",
          "title": "Another feature",
          "html_url": "https://github.com/octocat/Hello-World/pull/1348",
          "user": {
            "login": "octocat",
            "id": 1,
            "type": "User"
          },
          "created_at": "2011-01-26T19:01:12Z",
          "updated_at": "2011-01-26T19:01:12Z",
          "head": {
            "label": "octocat:different-branch",
            "ref_name": "different-branch",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          },
          "base": {
            "label": "octocat:master",
            "ref_name": "master",
            "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
          }
        }
      ])))
      .mount(&mock_server)
      .await;

    // Test finding pull requests by head branch
    let prs = client
      .find_pull_requests_by_head_branch("octocat", "Hello-World", "target-branch", Some("open"))
      .await?;

    // Verify we only got the PR with the matching head branch
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 1347);
    assert_eq!(prs[0].title, "Feature from target-branch");
    assert_eq!(prs[0].head.ref_name, Some("target-branch".to_string()));

    Ok(())
  }

  #[tokio::test]
  async fn test_get_pr_status() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    let pr_number = 1347;
    let commit_sha = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

    // Mock response for the pull request
    Mock::given(method("GET"))
      .and(path(format!("/repos/octocat/Hello-World/pulls/{}", pr_number)))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "id": 1,
        "number": pr_number,
        "state": "open",
        "title": "Amazing new feature",
        "html_url": "https://github.com/octocat/Hello-World/pull/1347",
        "user": {
          "login": "octocat",
          "id": 1,
          "name": "The Octocat"
        },
        "created_at": "2011-01-26T19:01:12Z",
        "updated_at": "2011-01-26T19:01:12Z",
        "head": {
          "label": "octocat:new-feature",
          "ref_name": "new-feature",
          "sha": commit_sha
        },
        "base": {
          "label": "octocat:master",
          "ref_name": "master",
          "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e"
        },
        "mergeable": true,
        "mergeable_state": "clean",
        "draft": false
      })))
      .mount(&mock_server)
      .await;

    // Mock response for PR reviews
    Mock::given(method("GET"))
      .and(path(format!("/repos/octocat/Hello-World/pulls/{}/reviews", pr_number)))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
        {
          "id": 80,
          "user": {
            "login": "reviewer1",
            "id": 2,
            "name": "Reviewer One"
          },
          "state": "APPROVED",
          "submitted_at": "2011-01-26T19:01:12Z"
        },
        {
          "id": 81,
          "user": {
            "login": "reviewer2",
            "id": 3,
            "name": "Reviewer Two"
          },
          "state": "CHANGES_REQUESTED",
          "submitted_at": "2011-01-26T20:01:12Z"
        }
      ])))
      .mount(&mock_server)
      .await;

    // Mock response for check runs
    Mock::given(method("GET"))
      .and(path(format!(
        "/repos/octocat/Hello-World/commits/{}/check-runs",
        commit_sha
      )))
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

    // Test getting PR status
    let status = client.get_pr_status("octocat", "Hello-World", pr_number).await?;

    // Verify PR details
    assert_eq!(status.pr.number, pr_number);
    assert_eq!(status.pr.title, "Amazing new feature");
    assert_eq!(status.pr.state, "open");
    assert_eq!(status.pr.mergeable, Some(true));

    // Verify reviews
    assert_eq!(status.reviews.len(), 2);
    assert_eq!(status.reviews[0].user.login, "reviewer1");
    assert_eq!(status.reviews[0].state, "APPROVED");
    assert_eq!(status.reviews[1].user.login, "reviewer2");
    assert_eq!(status.reviews[1].state, "CHANGES_REQUESTED");

    // Verify check runs
    assert_eq!(status.check_runs.len(), 2);
    assert_eq!(status.check_runs[0].name, "test-suite");
    assert_eq!(status.check_runs[0].conclusion, Some("success".to_string()));
    assert_eq!(status.check_runs[1].name, "lint");
    assert_eq!(status.check_runs[1].conclusion, Some("failure".to_string()));

    Ok(())
  }
}
