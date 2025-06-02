//! # Jira Issue Endpoints
//!
//! Jira API endpoint implementations for issue operations,
//! including fetching, creating, and updating Jira issues.

use anyhow::{Context, Result};
use reqwest::{StatusCode, header};
use serde::Deserialize;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::client::JiraClient;
use crate::consts::USER_AGENT;
use crate::models::Issue;

/// Represents a Jira comment
#[derive(Debug, Deserialize)]
pub struct Comment {
  pub id: String,
  pub body: String,
  pub author: User,
  pub created: String,
}

/// Represents a Jira user
#[derive(Debug, Deserialize)]
pub struct User {
  pub name: String,
  pub display_name: String,
  #[serde(default)]
  pub email_address: Option<String>,
}

impl JiraClient {
  /// Get a Jira issue by key
  #[instrument(skip(self), level = "debug")]
  pub async fn get_issue(&self, issue_key: &str) -> Result<Issue> {
    let url = format!("{}/rest/api/2/issue/{}", self.base_url, issue_key);
    info!("Fetching Jira issue: {}", issue_key);
    trace!("Jira API URL: {}", url);

    let response = self
      .client
      .get(&url)
      .header(header::USER_AGENT, USER_AGENT)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to fetch Jira issue")?;

    let status = response.status();
    debug!("Jira API response status: {}", status);

    match status {
      StatusCode::OK => {
        info!("Successfully received Jira issue data");
        let issue = response.json::<Issue>().await.context("Failed to parse Jira issue")?;

        info!("Successfully fetched Jira issue: {}", issue_key);
        trace!("Issue summary: {}", issue.fields.summary);

        Ok(issue)
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing Jira API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your Jira credentials."
        ))
      }
      StatusCode::NOT_FOUND => {
        warn!("Jira issue not found: {}", issue_key);
        Err(anyhow::anyhow!("Issue {} not found", issue_key))
      }
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected Jira API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {} - {}", status, error_text))
      }
    }
  }

  /// List Jira issues with filtering options
  #[instrument(skip(self), level = "debug")]
  pub async fn list_issues(
    &self,
    project: Option<&str>,
    status: Option<&str>,
    assignee: Option<&str>,
    pagination_options: Option<(u32, u32)>, // (max_results, start_at)
  ) -> Result<Vec<Issue>> {
    let mut jql_parts = Vec::new();

    // Add project filter
    if let Some(project_key) = project {
      jql_parts.push(format!("project = {project_key}"));
    }

    // Add status filter
    if let Some(status_name) = status {
      jql_parts.push(format!("status = \"{status_name}\""));
    }

    // Add assignee filter
    if let Some(assignee_name) = assignee {
      if assignee_name == "me" {
        jql_parts.push("assignee = currentUser()".to_string());
      } else {
        jql_parts.push(format!("assignee = \"{assignee_name}\""));
      }
    }

    // Build JQL query
    let jql = if jql_parts.is_empty() {
      "order by updated DESC".to_string()
    } else {
      format!("{} order by updated DESC", jql_parts.join(" AND "))
    };

    info!("JQL query: {}", jql);

    // Set up pagination
    let (max_results, start_at) = pagination_options.unwrap_or((50, 0));

    // Build URL with query parameters
    let url = format!(
      "{}/rest/api/2/search?jql={}&maxResults={}&startAt={}",
      self.base_url,
      urlencoding::encode(&jql),
      max_results,
      start_at
    );

    trace!("Jira API URL: {}", url);

    // Send request
    let response = self
      .client
      .get(&url)
      .header(header::USER_AGENT, USER_AGENT)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .inspect_err(|e| {
        error!("Request failed: {:?}", e);
        if e.is_timeout() {
          error!("Request timed out");
        } else if e.is_connect() {
          error!("Connection failed");
        } else if e.is_request() {
          error!("Request configuration error");
        }
      })
      .context("Failed to fetch Jira issues")?;

    let status = response.status();
    debug!("Jira API response status: {}", status);

    match status {
      StatusCode::OK => {
        #[derive(Deserialize)]
        struct SearchResponse {
          issues: Vec<Issue>,
          #[allow(dead_code)]
          total: u32,
        }

        let search_response = response
          .json::<SearchResponse>()
          .await
          .context("Failed to parse Jira search response")?;

        info!("Successfully fetched {} Jira issues", search_response.issues.len());
        Ok(search_response.issues)
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing Jira API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your Jira credentials."
        ))
      }
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected Jira API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {} - {}", status, error_text))
      }
    }
  }

  /// Add a comment to a Jira issue
  #[instrument(skip(self, comment_text), level = "debug")]
  pub async fn add_comment(&self, issue_key: &str, comment_text: &str, dry_run: bool) -> Result<Option<Comment>> {
    info!("Adding comment to Jira issue: {}", issue_key);

    if dry_run {
      info!("Dry run: Would add comment to issue {}", issue_key);
      info!("Comment text: {}", comment_text);
      return Ok(None);
    }

    // Build the request body
    let body = serde_json::json!({
      "body": comment_text
    });

    // Build the URL
    let url = format!("{}/rest/api/2/issue/{}/comment", self.base_url, issue_key);

    trace!("Jira API URL: {}", url);

    // Send request
    let response = self
      .client
      .post(&url)
      .header(header::USER_AGENT, USER_AGENT)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .json(&body)
      .send()
      .await
      .context("Failed to add comment to Jira issue")?;

    let status = response.status();
    debug!("Jira API response status: {}", status);

    match status {
      StatusCode::CREATED => {
        let comment = response
          .json::<Comment>()
          .await
          .context("Failed to parse Jira comment response")?;

        info!("Successfully added comment to issue {}", issue_key);
        Ok(Some(comment))
      }
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
        warn!("Authentication failed when accessing Jira API");
        Err(anyhow::anyhow!(
          "Authentication failed. Please check your Jira credentials."
        ))
      }
      StatusCode::NOT_FOUND => {
        warn!("Jira issue not found: {}", issue_key);
        Err(anyhow::anyhow!("Issue {} not found", issue_key))
      }
      _ => {
        let error_text = response.text().await.unwrap_or_default();
        warn!("Unexpected Jira API error: HTTP {} - {}", status, error_text);
        Err(anyhow::anyhow!("Unexpected error: HTTP {} - {}", status, error_text))
      }
    }
  }
}
