use anyhow::{Context, Result};
use reqwest::StatusCode;

use crate::api::jira::client::JiraClient;
use crate::api::jira::models::JiraIssue;

impl JiraClient {
  /// Get a Jira issue by key
  pub async fn get_issue(&self, issue_key: &str) -> Result<JiraIssue> {
    let url = format!("{}/rest/api/2/issue/{}", self.base_url, issue_key);

    let response = self
      .client
      .get(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to fetch Jira issue")?;

    match response.status() {
      StatusCode::OK => {
        let issue = response
          .json::<JiraIssue>()
          .await
          .context("Failed to parse Jira issue")?;
        Ok(issue)
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(anyhow::anyhow!(
        "Authentication failed. Please check your Jira credentials."
      )),
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Issue {} not found", issue_key)),
      _ => Err(anyhow::anyhow!(
        "Unexpected error: HTTP {} - {}",
        response.status(),
        response.text().await.unwrap_or_default()
      )),
    }
  }
}
