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
  #[allow(dead_code)]
  pub user: GitHubUser,
  pub created_at: String,
  pub updated_at: String,
  pub head: GitHubPRRef,
  #[allow(dead_code)]
  pub base: GitHubPRRef,
  pub mergeable: Option<bool>,
  pub mergeable_state: Option<String>,
  pub draft: Option<bool>,
}

/// Represents a GitHub pull request reference (head or base)
#[derive(Debug, Deserialize)]
pub struct GitHubPRRef {
  #[allow(dead_code)]
  pub label: String,
  #[allow(dead_code)]
  pub ref_name: Option<String>,
  pub sha: String,
}

/// Represents a GitHub pull request review
#[derive(Debug, Deserialize)]
pub struct GitHubPRReview {
  #[allow(dead_code)]
  pub id: u64,
  pub user: GitHubUser,
  pub state: String,
  pub submitted_at: String,
}

/// Represents a GitHub check run
#[derive(Debug, Deserialize)]
pub struct GitHubCheckRun {
  #[allow(dead_code)]
  pub id: u64,
  pub name: String,
  pub status: String,
  pub conclusion: Option<String>,
  #[allow(dead_code)]
  pub started_at: String,
  #[allow(dead_code)]
  pub completed_at: Option<String>,
}

/// Represents a GitHub check suite
#[derive(Debug, Deserialize)]
pub struct GitHubCheckSuite {
  #[allow(dead_code)]
  pub id: u64,
  #[allow(dead_code)]
  pub status: String,
  #[allow(dead_code)]
  pub conclusion: Option<String>,
  #[allow(dead_code)]
  pub check_runs: Vec<GitHubCheckRun>,
}

/// Represents a GitHub PR status summary
#[derive(Debug)]
pub struct GitHubPRStatus {
  pub pr: GitHubPullRequest,
  pub reviews: Vec<GitHubPRReview>,
  pub check_runs: Vec<GitHubCheckRun>,
}
