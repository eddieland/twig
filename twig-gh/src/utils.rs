//! # GitHub Utility Functions
//!
//! Helper functions for GitHub URL parsing, repository information extraction,
//! and other common GitHub-related operations.

use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

use crate::client::GitHubClient;

static GITHUB_REPO_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com[/:]([^/]+)/([^/\.]+)").expect("Failed to compile GitHub repo regex"));

static GITHUB_PR_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").expect("Failed to compile GitHub PR regex"));

impl GitHubClient {
  /// Extract owner and repo from a GitHub URL
  pub fn extract_repo_info_from_url(&self, url: &str) -> Result<(String, String)> {
    // Match patterns like:
    // https://github.com/owner/repo
    // https://github.com/owner/repo.git
    // https://github.com/owner/repo/pull/123
    if let Some(captures) = GITHUB_REPO_REGEX.captures(url) {
      let owner = captures.get(1).unwrap().as_str().to_string();
      let repo = captures.get(2).unwrap().as_str().to_string();
      Ok((owner, repo))
    } else {
      Err(anyhow::anyhow!("Could not extract owner and repo from URL: {url}"))
    }
  }

  /// Extract PR number from a GitHub PR URL
  pub fn extract_pr_number_from_url(&self, url: &str) -> Result<u32> {
    // Match patterns like:
    // https://github.com/owner/repo/pull/123
    if let Some(captures) = GITHUB_PR_REGEX.captures(url) {
      let pr_str = captures.get(1).unwrap().as_str();
      let pr_number = pr_str
        .parse::<u32>()
        .with_context(|| format!("Failed to parse PR number '{pr_str}' as a valid integer"))?;
      Ok(pr_number)
    } else {
      Err(anyhow::anyhow!("Could not extract PR number from URL: {url}"))
    }
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
