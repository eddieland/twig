//! # GitHub Utility Functions
//!
//! Helper functions for GitHub URL parsing, repository information extraction,
//! and other common GitHub-related operations.

use anyhow::{Context, Result};

use crate::client::GitHubClient;

impl GitHubClient {
  /// Extract owner and repo from a GitHub URL
  pub fn extract_repo_info_from_url(&self, url: &str) -> Result<(String, String)> {
    // Match patterns like:
    // https://github.com/owner/repo
    // https://github.com/owner/repo.git
    // https://github.com/owner/repo/pull/123
    twig_core::git::extract_github_repo_from_url(url)
  }

  /// Extract PR number from a GitHub PR URL
  pub fn extract_pr_number_from_url(&self, url: &str) -> Result<u32> {
    // Match patterns like:
    // https://github.com/owner/repo/pull/123
    twig_core::git::extract_pr_number_from_url(url)
      .with_context(|| format!("Could not extract PR number from URL: {url}"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::GitHubAuth;

  fn create_test_client() -> GitHubClient {
    let auth = GitHubAuth {
      username: "test_user".to_string(),
      token: "test_token".to_string(),
    };
    GitHubClient::new(auth)
  }

  #[test]
  fn test_extract_repo_info_from_url_https() {
    let client = create_test_client();

    // Test standard HTTPS URL
    let result = client.extract_repo_info_from_url("https://github.com/omenien/twig");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn test_extract_repo_info_from_url_git() {
    let client = create_test_client();

    // Test git URL
    let result = client.extract_repo_info_from_url("https://github.com/omenien/twig.git");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn test_extract_repo_info_from_url_with_path() {
    let client = create_test_client();

    // Test URL with additional path components
    let result = client.extract_repo_info_from_url("https://github.com/omenien/twig/pull/123");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn test_extract_repo_info_from_url_ssh() {
    let client = create_test_client();

    // Test SSH URL
    let result = client.extract_repo_info_from_url("git@github.com:omenien/twig.git");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn test_extract_repo_info_from_url_invalid() {
    let client = create_test_client();

    // Test invalid URL
    let result = client.extract_repo_info_from_url("https://example.com/not-github");
    assert!(result.is_err());

    // Test malformed GitHub URL
    let result = client.extract_repo_info_from_url("https://github.com/only-owner");
    assert!(result.is_err());
  }

  #[test]
  fn test_extract_pr_number_from_url_valid() {
    let client = create_test_client();

    // Test valid PR URL
    let result = client.extract_pr_number_from_url("https://github.com/omenien/twig/pull/123");
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 123);
  }

  #[test]
  fn test_extract_pr_number_from_url_with_fragment() {
    let client = create_test_client();

    // Test PR URL with fragment
    let result = client.extract_pr_number_from_url("https://github.com/omenien/twig/pull/456#discussion_r123456789");
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 456);
  }

  #[test]
  fn test_extract_pr_number_from_url_invalid() {
    let client = create_test_client();

    // Test non-PR URL
    let result = client.extract_pr_number_from_url("https://github.com/omenien/twig");
    assert!(result.is_err());

    // Test URL with invalid PR number format
    let result = client.extract_pr_number_from_url("https://github.com/omenien/twig/pull/abc");
    assert!(result.is_err());
  }
}
