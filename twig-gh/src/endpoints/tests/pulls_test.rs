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
    assert_eq!(reviews[0].body.unwrap(), "LGTM");

    Ok(())
  }
}
