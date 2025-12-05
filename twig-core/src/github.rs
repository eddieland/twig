//! GitHub URL parsing helpers shared across crates.
//!
//! These helpers intentionally live in `twig-core` so both the CLI and service
//! clients can parse GitHub URLs without depending on a client instance.

use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

static GITHUB_REPO_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com[/:]([^/]+)/([^/\.]+)").expect("Failed to compile GitHub repo regex"));

static GITHUB_PR_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").expect("Failed to compile GitHub PR regex"));

/// Extract owner and repository name from a GitHub URL.
///
/// Supports HTTPS, SSH, and URLs containing additional path segments (e.g.,
/// pull request paths). Returns an error when the URL does not resemble a
/// GitHub repository path.
pub fn extract_repo_info_from_url(url: &str) -> Result<(String, String)> {
  if let Some(captures) = GITHUB_REPO_REGEX.captures(url) {
    let owner = captures.get(1).unwrap().as_str().to_string();
    let repo = captures.get(2).unwrap().as_str().to_string();
    Ok((owner, repo))
  } else {
    Err(anyhow::anyhow!("Could not extract owner and repo from URL: {url}"))
  }
}

/// Extract PR number from a GitHub PR URL.
///
/// Accepts standard pull request URLs and URLs with fragments or query
/// parameters. Returns an error if the URL does not contain a numeric pull
/// request identifier.
pub fn extract_pr_number_from_url(url: &str) -> Result<u32> {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_repo_info_from_url_https() {
    let result = extract_repo_info_from_url("https://github.com/omenien/twig");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_git_suffix_and_trailing_slash() {
    let result = extract_repo_info_from_url("https://github.com/omenien/twig.git/");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_with_path() {
    let result = extract_repo_info_from_url("https://github.com/omenien/twig/pull/123");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_ssh() {
    let result = extract_repo_info_from_url("git@github.com:omenien/twig.git");
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_invalid() {
    let result = extract_repo_info_from_url("https://example.com/not-github");
    assert!(result.is_err());

    let result = extract_repo_info_from_url("https://github.com/only-owner");
    assert!(result.is_err());
  }

  #[test]
  fn extract_pr_number_from_url_valid() {
    let result = extract_pr_number_from_url("https://github.com/omenien/twig/pull/123");
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 123);
  }

  #[test]
  fn extract_pr_number_from_url_with_fragment_and_query() {
    let result = extract_pr_number_from_url("https://github.com/omenien/twig/pull/456#discussion_r123456789");
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 456);

    let result = extract_pr_number_from_url("https://github.com/omenien/twig/pull/456?utm_source=test");
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 456);
  }

  #[test]
  fn extract_pr_number_from_url_invalid() {
    let result = extract_pr_number_from_url("https://github.com/omenien/twig");
    assert!(result.is_err());

    let result = extract_pr_number_from_url("https://github.com/omenien/twig/pull/abc");
    assert!(result.is_err());
  }
}
