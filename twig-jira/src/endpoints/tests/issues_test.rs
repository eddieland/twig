#[cfg(test)]
mod tests {
  use wiremock::matchers::{basic_auth, header, method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use crate::client::JiraClient;
  use crate::models::JiraAuth;

  #[tokio::test]
  async fn test_get_issue() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock response for issue
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/TEST-123"))
      .and(basic_auth("test_user", Some("test_token")))
      .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
          "id": "10000",
          "key": "TEST-123",
          "fields": {
              "summary": "Test issue",
              "description": "This is a test issue",
              "status": {
                  "id": "10001",
                  "name": "In Progress",
                  "statusCategory": {
                      "id": 4,
                      "key": "indeterminate",
                      "name": "In Progress"
                  }
              }
          }
      })))
      .mount(&mock_server)
      .await;

    let issue = client.get_issue("TEST-123").await?;
    assert_eq!(issue.key, "TEST-123");
    assert_eq!(issue.fields.summary, "Test issue");
    assert_eq!(issue.fields.status.name, "In Progress");

    Ok(())
  }

  #[tokio::test]
  async fn test_get_issue_not_found() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "test_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock 404 response
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/NONEXISTENT-123"))
      .and(basic_auth("test_user", Some("test_token")))
      .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
          "errorMessages": ["Issue does not exist or you do not have permission to see it."],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_issue("NONEXISTENT-123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    Ok(())
  }

  #[tokio::test]
  async fn test_get_issue_unauthorized() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let auth = JiraAuth {
      username: "test_user".to_string(),
      api_token: "invalid_token".to_string(),
    };
    let base_url = mock_server.uri();
    let client = JiraClient::new(&base_url, auth);

    // Mock unauthorized response
    Mock::given(method("GET"))
      .and(path("/rest/api/2/issue/TEST-123"))
      .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
          "errorMessages": ["Authentication failed"],
          "errors": {}
      })))
      .mount(&mock_server)
      .await;

    let result = client.get_issue("TEST-123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Authentication failed"));

    Ok(())
  }
}
