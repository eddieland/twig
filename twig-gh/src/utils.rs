use anyhow::{Context, Result};

use crate::client::GitHubClient;

impl GitHubClient {
  /// Extract owner and repo from a GitHub URL
  pub fn extract_repo_info_from_url(&self, url: &str) -> Result<(String, String)> {
    // Match patterns like:
    // https://github.com/owner/repo
    // https://github.com/owner/repo.git
    // https://github.com/owner/repo/pull/123
    let re = regex::Regex::new(r"github\.com[/:]([^/]+)/([^/\.]+)").context("Failed to compile regex")?;

    if let Some(captures) = re.captures(url) {
      let owner = captures.get(1).unwrap().as_str().to_string();
      let repo = captures.get(2).unwrap().as_str().to_string();
      Ok((owner, repo))
    } else {
      Err(anyhow::anyhow!("Could not extract owner and repo from URL: {}", url))
    }
  }

  /// Extract PR number from a GitHub PR URL
  pub fn extract_pr_number_from_url(&self, url: &str) -> Result<u32> {
    // Match patterns like:
    // https://github.com/owner/repo/pull/123
    let re = regex::Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").context("Failed to compile regex")?;

    if let Some(captures) = re.captures(url) {
      let pr_str = captures.get(1).unwrap().as_str();
      let pr_number = pr_str
        .parse::<u32>()
        .with_context(|| format!("Failed to parse PR number '{pr_str}' as a valid integer"))?;
      Ok(pr_number)
    } else {
      Err(anyhow::anyhow!("Could not extract PR number from URL: {}", url))
    }
  }
}
