#![allow(dead_code)]

use serde::Deserialize;

/// Represents GitHub authentication credentials
#[derive(Clone)]
pub struct GitHubAuth {
  pub username: String,
  pub token: String,
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
  pub user: GitHubUser,
  pub created_at: String,
  pub updated_at: String,
  pub head: GitHubPRRef,
  pub base: GitHubPRRef,
  pub mergeable: Option<bool>,
  pub mergeable_state: Option<String>,
  pub draft: Option<bool>,
}

/// Represents a GitHub pull request reference (head or base)
#[derive(Debug, Deserialize)]
pub struct GitHubPRRef {
  pub label: String,
  pub ref_name: Option<String>,
  pub sha: String,
}

/// Represents a GitHub pull request review
#[derive(Debug, Deserialize)]
pub struct GitHubPRReview {
  pub id: u64,
  pub user: GitHubUser,
  pub state: String,
  pub submitted_at: String,
}

/// Represents a GitHub check run
#[derive(Debug, Deserialize)]
pub struct GitHubCheckRun {
  pub id: u64,
  pub name: String,
  pub status: String,
  pub conclusion: Option<String>,
  pub started_at: String,
  pub completed_at: Option<String>,
}

/// Represents a GitHub check suite
#[derive(Debug, Deserialize)]
pub struct GitHubCheckSuite {
  pub id: u64,
  pub status: String,
  pub conclusion: Option<String>,
  pub check_runs: Vec<GitHubCheckRun>,
}

/// Represents a GitHub PR status summary
#[derive(Debug)]
pub struct GitHubPRStatus {
  pub pr: GitHubPullRequest,
  pub reviews: Vec<GitHubPRReview>,
  pub check_runs: Vec<GitHubCheckRun>,
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn test_github_auth() {
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };

    assert_eq!(auth.username, "test_user");
    assert_eq!(auth.token, "test_token");
  }

  #[test]
  fn test_github_user_deserialization() {
    let json = json!({
        "login": "octocat",
        "id": 1,
        "name": "The Octocat"
    });

    let user: GitHubUser = serde_json::from_value(json).unwrap();

    assert_eq!(user.login, "octocat");
    assert_eq!(user.id, 1);
    assert_eq!(user.name, Some("The Octocat".to_string()));
  }

  #[test]
  fn test_github_pull_request_deserialization() {
    let json = json!({
        "number": 1347,
        "title": "Amazing new feature",
        "html_url": "https://github.com/octocat/Hello-World/pull/1347",
        "state": "open",
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
    });

    let pr: GitHubPullRequest = serde_json::from_value(json).unwrap();

    assert_eq!(pr.number, 1347);
    assert_eq!(pr.title, "Amazing new feature");
    assert_eq!(pr.state, "open");
    assert_eq!(pr.mergeable, Some(true));
    assert_eq!(pr.draft, Some(false));
  }

  #[test]
  fn test_github_pr_review_deserialization() {
    let json = json!({
        "id": 80,
        "user": {
            "login": "octocat",
            "id": 1,
            "name": "The Octocat"
        },
        "state": "APPROVED",
        "submitted_at": "2011-01-26T19:01:12Z"
    });

    let review: GitHubPRReview = serde_json::from_value(json).unwrap();

    assert_eq!(review.id, 80);
    assert_eq!(review.state, "APPROVED");
    assert_eq!(review.user.login, "octocat");
  }

  #[test]
  fn test_github_check_run_deserialization() {
    let json = json!({
        "id": 4,
        "name": "test-suite",
        "status": "completed",
        "conclusion": "success",
        "started_at": "2011-01-26T19:01:12Z",
        "completed_at": "2011-01-26T19:01:12Z"
    });

    let check: GitHubCheckRun = serde_json::from_value(json).unwrap();

    assert_eq!(check.id, 4);
    assert_eq!(check.name, "test-suite");
    assert_eq!(check.status, "completed");
    assert_eq!(check.conclusion, Some("success".to_string()));
  }
}
