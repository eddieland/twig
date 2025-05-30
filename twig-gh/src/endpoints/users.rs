use anyhow::{Context, Result};
use reqwest::StatusCode;
use tracing::instrument;

use crate::client::GitHubClient;
use crate::models::GitHubUser;

impl GitHubClient {
  /// Get the current authenticated user
  #[instrument(skip(self), level = "debug")]
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
#[cfg(test)]
mod tests {
  use wiremock::matchers::{header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::client::GitHubClient;
  use crate::models::GitHubAuth;

  #[tokio::test]
  async fn test_get_current_user() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock response for current user
    Mock::given(method("GET"))
      .and(path("/user"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .and(header("Authorization", "Basic dGVzdF91c2VyOnRlc3RfdG9rZW4="))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "login": "test_user",
          "id": 1,
          "name": "Test User",
          "email": "test@example.com",
          "avatar_url": "https://github.com/images/test.png",
          "html_url": "https://github.com/test_user"
      })))
      .mount(&mock_server)
      .await;

    let user = client.get_current_user().await?;
    assert_eq!(user.login, "test_user");
    assert_eq!(user.id, 1);
    assert_eq!(user.name, Some("Test User".to_string()));

    Ok(())
  }

  #[tokio::test]
  async fn test_get_current_user_unauthorized() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "invalid_token".to_string(),
    };
    let mut client = GitHubClient::new(auth);
    client.base_url = mock_server.uri();

    // Mock unauthorized response
    Mock::given(method("GET"))
      .and(path("/user"))
      .and(header("Accept", "application/vnd.github.v3+json"))
      .and(header("User-Agent", "twig-cli"))
      .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
          "message": "Bad credentials",
          "documentation_url": "https://docs.github.com/rest"
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_current_user().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Authentication failed"));

    Ok(())
  }
}
