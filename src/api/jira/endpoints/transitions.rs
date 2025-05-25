use anyhow::{Context, Result};
use reqwest::StatusCode;

use crate::api::jira::client::JiraClient;
use crate::api::jira::models::{JiraTransition, JiraTransitions, TransitionId, TransitionRequest};

impl JiraClient {
  /// Get available transitions for an issue
  pub async fn get_transitions(&self, issue_key: &str) -> Result<Vec<JiraTransition>> {
    let url = format!("{}/rest/api/2/issue/{}/transitions", self.base_url, issue_key);

    let response = self
      .client
      .get(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .send()
      .await
      .context("Failed to fetch Jira transitions")?;

    match response.status() {
      StatusCode::OK => {
        let transitions = response
          .json::<JiraTransitions>()
          .await
          .context("Failed to parse Jira transitions")?;
        Ok(transitions.transitions)
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

  /// Transition an issue to a new status
  pub async fn transition_issue(&self, issue_key: &str, transition_id: &str) -> Result<()> {
    let url = format!("{}/rest/api/2/issue/{}/transitions", self.base_url, issue_key);

    let payload = TransitionRequest {
      transition: TransitionId {
        id: transition_id.to_string(),
      },
    };

    let response = self
      .client
      .post(&url)
      .basic_auth(&self.auth.username, Some(&self.auth.api_token))
      .json(&payload)
      .send()
      .await
      .context("Failed to transition Jira issue")?;

    match response.status() {
      StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(anyhow::anyhow!(
        "Authentication failed. Please check your Jira credentials."
      )),
      StatusCode::NOT_FOUND => Err(anyhow::anyhow!("Issue {} not found", issue_key)),
      StatusCode::BAD_REQUEST => Err(anyhow::anyhow!(
        "Invalid transition. The transition may not be available for the current status."
      )),
      _ => Err(anyhow::anyhow!(
        "Unexpected error: HTTP {} - {}",
        response.status(),
        response.text().await.unwrap_or_default()
      )),
    }
  }
}
