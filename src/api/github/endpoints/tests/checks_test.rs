#[cfg(test)]
mod tests {
  use wiremock::matchers::{header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::api::github::client::GitHubClient;
  use crate::api::github::models::GitHubAuth;

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
