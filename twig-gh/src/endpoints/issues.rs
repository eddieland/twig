//! GitHub Issues API endpoint implementations.

use anyhow::{Context, Result};
use reqwest::header;
use tracing::{debug, info, instrument, trace, warn};

use crate::client::GitHubClient;
use crate::consts::{ACCEPT, USER_AGENT};
use crate::models::GitHubIssue;

impl GitHubClient {
  /// Get a specific issue by number.
  ///
  /// # Errors
  ///
  /// Returns an error if the issue is not found, authentication fails,
  /// the request cannot be sent, or the response cannot be parsed.
  #[instrument(skip(self), level = "debug")]
  pub async fn get_issue(&self, owner: &str, repo: &str, issue_number: u32) -> Result<GitHubIssue> {
    info!("Fetching issue #{} for {}/{}", issue_number, owner, repo);

    let url = format!("{}/repos/{}/{}/issues/{}", self.base_url, owner, repo, issue_number);

    trace!("GitHub API URL: {}", url);

    let response = self
      .client
      .get(&url)
      .header(header::ACCEPT, ACCEPT)
      .header(header::USER_AGENT, USER_AGENT)
      .basic_auth(&self.auth.username, Some(&self.auth.token))
      .send()
      .await
      .context(format!("GET {url} failed"))?;

    let status = response.status();
    debug!("GitHub API response status: {}", status);

    match status {
      reqwest::StatusCode::OK => {
        info!("Successfully received issue data");
        let issue = response
          .json::<GitHubIssue>()
          .await
          .context("Failed to parse GitHub issue response")?;
        trace!("Issue: {:?}", issue);
        Ok(issue)
      }
      reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing GitHub API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your GitHub credentials."
        ))
      }
      reqwest::StatusCode::NOT_FOUND => Err(anyhow::anyhow!(
        "Issue #{} not found for {}/{}",
        issue_number,
        owner,
        repo
      )),
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected GitHub API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {status} - {error_text}"))
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;
  use wiremock::matchers::{header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::*;
  use crate::models::GitHubAuth;

  #[tokio::test]
  async fn test_get_issue_success() -> Result<()> {
    let mock_server = MockServer::start().await;

    let mock_issue = json!({
      "number": 123,
      "title": "Test Issue",
      "body": "This is a test issue",
      "html_url": "https://github.com/owner/repo/issues/123",
      "state": "open",
      "user": {
        "login": "test_user",
        "id": 12345,
        "name": "Test User"
      },
      "created_at": "2023-01-01T12:00:00Z",
      "updated_at": "2023-01-01T12:00:00Z",
      "labels": [],
      "assignees": []
    });

    Mock::given(method("GET"))
      .and(path("/repos/owner/repo/issues/123"))
      .and(header("accept", ACCEPT))
      .and(header("user-agent", USER_AGENT))
      .respond_with(ResponseTemplate::new(200).set_body_json(&mock_issue))
      .mount(&mock_server)
      .await;

    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };

    let mut client = crate::client::GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    let issue = client.get_issue("owner", "repo", 123).await?;

    assert_eq!(issue.number, 123);
    assert_eq!(issue.title, "Test Issue");
    assert_eq!(issue.state, "open");

    Ok(())
  }

  #[tokio::test]
  async fn test_get_issue_not_found() -> Result<()> {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
      .and(path("/repos/owner/repo/issues/404"))
      .respond_with(ResponseTemplate::new(404))
      .mount(&mock_server)
      .await;

    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };

    let mut client = crate::client::GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    let result = client.get_issue("owner", "repo", 404).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Issue #404 not found"));

    Ok(())
  }
}
