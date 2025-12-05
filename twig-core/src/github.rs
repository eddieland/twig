//! GitHub URL parsing helpers shared across crates.
//!
//! These helpers intentionally live in `twig-core` so both the CLI and service
//! clients can parse GitHub URLs without depending on a client instance. Hosts
//! are validated against a configurable allowlist to support GitHub Enterprise
//! without over-matching unrelated URLs.

use anyhow::{Context, Result};
use url::Url;

use crate::config::{ConfigDirs, GitHubConfig};

/// Extract owner and repository name from a GitHub URL using the configured
/// host allowlist.
///
/// Supports HTTPS, SSH, file-based mirrors, and URLs containing additional
/// path segments (e.g., pull request paths). Returns an error when the URL
/// does not resemble a GitHub repository path or the host is not in the
/// configured allowlist.
pub fn extract_repo_info_from_url(url: &str) -> Result<(String, String)> {
  extract_repo_info_from_url_with_hosts(url, &default_github_hosts())
}

/// Extract PR number from a GitHub PR URL using the configured host allowlist.
///
/// Accepts standard pull request URLs and URLs with fragments or query
/// parameters. Returns an error if the URL does not contain a numeric pull
/// request identifier or the host is not in the configured allowlist.
pub fn extract_pr_number_from_url(url: &str) -> Result<u32> {
  extract_pr_number_from_url_with_hosts(url, &default_github_hosts())
}

/// Extract owner and repository name from a GitHub URL using the provided host
/// allowlist.
pub fn extract_repo_info_from_url_with_hosts(url: &str, hosts: &[String]) -> Result<(String, String)> {
  if let Some((owner, repo)) = extract_owner_repo_from_file_url(url, hosts)? {
    return Ok((owner, repo));
  }

  let (host, path) = extract_host_and_path(url)?;
  ensure_host_allowed(&host, hosts)?;

  extract_owner_repo_from_path(&path).ok_or_else(|| anyhow::anyhow!("Could not extract owner and repo from URL: {url}"))
}

/// Extract PR number from a GitHub PR URL using the provided host allowlist.
pub fn extract_pr_number_from_url_with_hosts(url: &str, hosts: &[String]) -> Result<u32> {
  let (host, path) = extract_host_and_path(url)?;
  ensure_host_allowed(&host, hosts)?;

  extract_pr_number_from_path(&path).with_context(|| format!("Could not extract PR number from URL: {url}"))
}

fn extract_host_and_path(url: &str) -> Result<(String, String)> {
  if let Ok(parsed) = Url::parse(url)
    && let Some(host) = parsed.host_str()
  {
    return Ok((host.to_string(), parsed.path().to_string()));
  }

  // SSH-style: git@host:owner/repo.git
  if let Some(stripped) = url.strip_prefix("git@")
    && let Some((host, path)) = stripped.split_once(':')
  {
    return Ok((host.to_string(), format!("/{}", path)));
  }

  Err(anyhow::anyhow!("Unsupported GitHub URL format: {url}"))
}

fn ensure_host_allowed(host: &str, allowed: &[String]) -> Result<()> {
  if allowed.iter().any(|h| h.eq_ignore_ascii_case(host)) {
    Ok(())
  } else {
    Err(anyhow::anyhow!(
      "Host '{host}' is not in the configured GitHub host allowlist"
    ))
  }
}

fn extract_owner_repo_from_path(path: &str) -> Option<(String, String)> {
  let mut segments = path.trim_start_matches('/').split('/');
  let owner = segments.next()?;
  let repo_raw = segments.next()?;
  let repo = repo_raw.trim_end_matches(".git");
  if owner.is_empty() || repo.is_empty() {
    return None;
  }
  Some((owner.to_string(), repo.to_string()))
}

fn extract_owner_repo_from_file_url(url: &str, allowed_hosts: &[String]) -> Result<Option<(String, String)>> {
  let parsed = match Url::parse(url) {
    Ok(parsed) if parsed.scheme() == "file" => parsed,
    _ => return Ok(None),
  };

  let segments: Vec<_> = parsed
    .path_segments()
    .map(|iter| iter.filter(|seg| !seg.is_empty()).collect())
    .unwrap_or_default();

  let host_index = segments
    .iter()
    .position(|seg| allowed_hosts.iter().any(|allowed| allowed.eq_ignore_ascii_case(seg)));

  if let Some(idx) = host_index
    && segments.len() > idx + 2
  {
    let owner = segments[idx + 1].to_string();
    let repo_raw = segments[idx + 2];
    let repo = repo_raw.trim_end_matches(".git");
    if !owner.is_empty() && !repo.is_empty() {
      return Ok(Some((owner, repo.to_string())));
    }
  }

  Ok(None)
}

fn extract_pr_number_from_path(path: &str) -> Option<u32> {
  let mut segments = path.trim_start_matches('/').split('/');
  while let Some(seg) = segments.next() {
    if seg == "pull"
      && let Some(id) = segments.next()
    {
      return id.parse::<u32>().ok();
    }
  }
  None
}

fn default_github_hosts() -> Vec<String> {
  ConfigDirs::new()
    .ok()
    .and_then(|dirs| dirs.load_github_config().ok())
    .filter(|cfg| !cfg.hosts.is_empty())
    .map(|cfg| cfg.hosts)
    .unwrap_or_else(|| GitHubConfig::default().hosts)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_repo_info_from_url_https() {
    let result = extract_repo_info_from_url_with_hosts("https://github.com/omenien/twig", &["github.com".into()]);
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_git_suffix_and_trailing_slash() {
    let result = extract_repo_info_from_url_with_hosts("https://github.com/omenien/twig.git/", &["github.com".into()]);
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_with_path() {
    let result =
      extract_repo_info_from_url_with_hosts("https://github.com/omenien/twig/pull/123", &["github.com".into()]);
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_ssh() {
    let result = extract_repo_info_from_url_with_hosts("git@github.com:omenien/twig.git", &["github.com".into()]);
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "omenien");
    assert_eq!(repo, "twig");
  }

  #[test]
  fn extract_repo_info_from_url_custom_host() {
    let hosts = vec!["github.example.com".to_string()];
    let result = extract_repo_info_from_url_with_hosts("https://github.example.com/org/repo", &hosts);
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "org");
    assert_eq!(repo, "repo");
  }

  #[test]
  fn extract_repo_info_from_file_url_with_mirror() {
    let result = extract_repo_info_from_url_with_hosts("file:///tmp/github.com/org/repo", &["github.com".into()]);
    assert!(result.is_ok());
    let (owner, repo) = result.unwrap();
    assert_eq!(owner, "org");
    assert_eq!(repo, "repo");
  }

  #[test]
  fn extract_repo_info_from_url_invalid_host() {
    let result = extract_repo_info_from_url_with_hosts("https://notgithub.com/org/repo", &["github.com".into()]);
    assert!(result.is_err());
  }

  #[test]
  fn extract_repo_info_from_url_invalid() {
    let result = extract_repo_info_from_url_with_hosts("https://example.com/not-github", &["github.com".into()]);
    assert!(result.is_err());

    let result = extract_repo_info_from_url_with_hosts("https://github.com/only-owner", &["github.com".into()]);
    assert!(result.is_err());
  }

  #[test]
  fn extract_pr_number_from_url_valid() {
    let result =
      extract_pr_number_from_url_with_hosts("https://github.com/omenien/twig/pull/123", &["github.com".into()]);
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 123);
  }

  #[test]
  fn extract_pr_number_from_url_with_fragment_and_query() {
    let result = extract_pr_number_from_url_with_hosts(
      "https://github.com/omenien/twig/pull/456#discussion_r123456789",
      &["github.com".into()],
    );
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 456);

    let result = extract_pr_number_from_url_with_hosts(
      "https://github.com/omenien/twig/pull/456?utm_source=test",
      &["github.com".into()],
    );
    assert!(result.is_ok());
    let pr_number = result.unwrap();
    assert_eq!(pr_number, 456);
  }

  #[test]
  fn extract_pr_number_from_custom_host() {
    let hosts = vec!["github.example.com".to_string()];
    let result = extract_pr_number_from_url_with_hosts("https://github.example.com/org/repo/pull/777", &hosts).unwrap();
    assert_eq!(result, 777);
  }

  #[test]
  fn extract_pr_number_from_url_invalid() {
    let result = extract_pr_number_from_url_with_hosts("https://github.com/omenien/twig", &["github.com".into()]);
    assert!(result.is_err());

    let result =
      extract_pr_number_from_url_with_hosts("https://github.com/omenien/twig/pull/abc", &["github.com".into()]);
    assert!(result.is_err());
  }
}
