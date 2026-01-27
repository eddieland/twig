//! URL parsing helpers shared across crates.
//!
//! These helpers live in `twig-core` so both the CLI and service clients can
//! parse URLs without depending on a specific client instance.

use std::path::Path;

use anyhow::{Context, Result};
use git2::Repository as Git2Repository;
use url::{Position, Url};

use crate::extract_repo_info_from_url;

/// Environment variable storing the Jira host configuration.
pub const ENV_JIRA_HOST: &str = "JIRA_HOST";

/// Get the $JIRA_HOST environment variable value with proper URL scheme.
///
/// If the host doesn't include a scheme (http:// or https://), assumes https://.
/// Returns an error if the environment variable is not set.
pub fn resolve_jira_base_url() -> Result<String> {
  let jira_host = std::env::var(ENV_JIRA_HOST);
  match jira_host {
    Ok(host) => Ok(ensure_url_scheme(&host)?),
    Err(_) => Err(anyhow::anyhow!(
      "Jira host environment variable '{ENV_JIRA_HOST}' not set"
    )),
  }
}

/// Extract owner and repository name from a git repository's origin remote.
///
/// Opens the repository at the given path, finds the "origin" remote, and
/// extracts the owner/repo information from the remote URL.
pub fn resolve_github_repo<P: AsRef<Path>>(repo_path: P) -> Result<(String, String)> {
  let repo = Git2Repository::open(repo_path.as_ref())
    .with_context(|| format!("Failed to open git repository at {}", repo_path.as_ref().display()))?;

  resolve_github_repo_from_git2(&repo)
}

/// Extract owner and repository name from an open git2 Repository.
///
/// Finds the "origin" remote and extracts the owner/repo information
/// from the remote URL.
pub fn resolve_github_repo_from_git2(repo: &Git2Repository) -> Result<(String, String)> {
  let remote = repo.find_remote("origin").context("Failed to find remote 'origin'")?;

  let remote_url = remote.url().context("Remote 'origin' has no URL")?;

  extract_repo_info_from_url(remote_url)
}

/// Normalize a URL by removing trailing slashes from the path when it's just
/// "/".
fn normalize_url(url: &Url) -> String {
  let mut result = String::new();
  result.push_str(&url[..Position::BeforePath]);

  let path = url.path();
  if path != "/" {
    result.push_str(path);
  }

  if let Some(query) = url.query() {
    result.push('?');
    result.push_str(query);
  }

  if let Some(fragment) = url.fragment() {
    result.push('#');
    result.push_str(fragment);
  }

  result
}

/// Parse a URL by prefixing it with https:// scheme.
fn parse_with_https_prefix(input: &str) -> Result<Url> {
  let mut candidate = input;

  if let Some(colon_index) = input.find(':') {
    let potential_scheme = &input[..colon_index];
    if ["http", "https"]
      .iter()
      .any(|scheme| potential_scheme.eq_ignore_ascii_case(scheme))
    {
      let remainder = input[colon_index + 1..].trim_start_matches('/');
      if !remainder.is_empty() {
        candidate = remainder;
      }
    }
  }

  let with_scheme = format!("https://{candidate}");
  Url::parse(&with_scheme).map_err(|_| anyhow::anyhow!("Failed to parse URL: '{input}'. Ensure it has a valid scheme."))
}

/// Ensure a URL has a proper scheme (http:// or https://).
///
/// If the input doesn't include a scheme, assumes https://. Also handles
/// malformed schemes like "http:/example.com" (missing slash).
pub fn ensure_url_scheme(input: &str) -> Result<String> {
  let trimmed = input.trim();
  if trimmed.is_empty() {
    return Err(anyhow::anyhow!("Host cannot be empty"));
  }

  let lowered = trimmed.to_ascii_lowercase();
  if lowered.starts_with("http:") && !lowered.starts_with("http://") {
    let remainder = trimmed.split_once(':').map(|(_, rest)| rest).unwrap_or("");
    return parse_with_https_prefix(remainder.trim_start_matches('/')).map(|url| normalize_url(&url));
  }

  if lowered.starts_with("https:") && !lowered.starts_with("https://") {
    let remainder = trimmed.split_once(':').map(|(_, rest)| rest).unwrap_or("");
    return parse_with_https_prefix(remainder.trim_start_matches('/')).map(|url| normalize_url(&url));
  }

  let url = if let Ok(url) = Url::parse(trimmed) {
    if url.scheme().len() > 1 && url.host().is_some() {
      url
    } else {
      parse_with_https_prefix(trimmed)?
    }
  } else {
    parse_with_https_prefix(trimmed)?
  };

  Ok(normalize_url(&url))
}

#[cfg(test)]
mod tests {
  use twig_test_utils::{EnvVarGuard, GitRepoTestGuard, create_commit};

  use super::*;

  // Tests for ensure_url_scheme

  #[test]
  fn test_ensure_url_scheme_with_https() {
    let result = ensure_url_scheme("https://company.atlassian.net").unwrap();
    assert_eq!(result, "https://company.atlassian.net");
  }

  #[test]
  fn test_ensure_url_scheme_with_http() {
    let result = ensure_url_scheme("http://jira.example.com").unwrap();
    assert_eq!(result, "http://jira.example.com");
  }

  #[test]
  fn test_ensure_url_scheme_without_scheme() {
    let result = ensure_url_scheme("company.atlassian.net").unwrap();
    assert_eq!(result, "https://company.atlassian.net");
  }

  #[test]
  fn test_ensure_url_scheme_with_subdomain() {
    let result = ensure_url_scheme("my-company.atlassian.net").unwrap();
    assert_eq!(result, "https://my-company.atlassian.net");
  }

  #[test]
  fn test_ensure_url_scheme_empty_string() {
    let result = ensure_url_scheme("");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Host cannot be empty"));
  }

  #[test]
  fn test_ensure_url_scheme_whitespace_only() {
    let result = ensure_url_scheme("   ");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Host cannot be empty"));
  }

  #[test]
  fn test_ensure_url_scheme_with_port() {
    let result = ensure_url_scheme("localhost:8080").unwrap();
    assert_eq!(result, "https://localhost:8080");
  }

  #[test]
  fn test_ensure_url_scheme_with_http_and_port() {
    let result = ensure_url_scheme("http://localhost:8080").unwrap();
    assert_eq!(result, "http://localhost:8080");
  }

  #[test]
  fn test_ensure_url_scheme_with_https_and_port() {
    let result = ensure_url_scheme("https://localhost:9443").unwrap();
    assert_eq!(result, "https://localhost:9443");
  }

  #[test]
  fn test_ensure_url_scheme_ip_address() {
    let result = ensure_url_scheme("192.168.1.100").unwrap();
    assert_eq!(result, "https://192.168.1.100");
  }

  #[test]
  fn test_ensure_url_scheme_ip_address_with_port() {
    let result = ensure_url_scheme("192.168.1.100:8080").unwrap();
    assert_eq!(result, "https://192.168.1.100:8080");
  }

  #[test]
  fn test_ensure_url_scheme_localhost() {
    let result = ensure_url_scheme("localhost").unwrap();
    assert_eq!(result, "https://localhost");
  }

  #[test]
  fn test_ensure_url_scheme_with_path() {
    let result = ensure_url_scheme("example.com/path/to/resource").unwrap();
    assert_eq!(result, "https://example.com/path/to/resource");
  }

  #[test]
  fn test_ensure_url_scheme_with_query_params() {
    let result = ensure_url_scheme("example.com?param=value").unwrap();
    assert_eq!(result, "https://example.com?param=value");
  }

  #[test]
  fn test_ensure_url_scheme_with_fragment() {
    let result = ensure_url_scheme("example.com#section").unwrap();
    assert_eq!(result, "https://example.com#section");
  }

  #[test]
  fn test_ensure_url_scheme_case_sensitivity() {
    let result = ensure_url_scheme("HTTP://example.com").unwrap();
    assert_eq!(result, "http://example.com");
  }

  #[test]
  fn test_ensure_url_scheme_case_sensitivity_https() {
    let result = ensure_url_scheme("HTTPS://example.com").unwrap();
    assert_eq!(result, "https://example.com");
  }

  #[test]
  fn test_ensure_url_scheme_partial_scheme_http() {
    let result = ensure_url_scheme("http:/example.com").unwrap();
    assert_eq!(result, "https://example.com");
  }

  #[test]
  fn test_ensure_url_scheme_partial_scheme_https() {
    let result = ensure_url_scheme("https:/example.com").unwrap();
    assert_eq!(result, "https://example.com");
  }

  #[test]
  fn test_ensure_url_scheme_scheme_in_middle() {
    let result = ensure_url_scheme("example.com/http://other.com").unwrap();
    assert_eq!(result, "https://example.com/http://other.com");
  }

  #[test]
  fn test_ensure_url_scheme_ftp_protocol() {
    let result = ensure_url_scheme("ftp://example.com").unwrap();
    assert_eq!(result, "ftp://example.com");
  }

  // Tests for resolve_jira_base_url

  #[test]
  fn test_resolve_jira_base_url_with_env_var() {
    let guard = EnvVarGuard::new(ENV_JIRA_HOST);
    guard.set("company.atlassian.net");

    let result = resolve_jira_base_url().unwrap();
    assert_eq!(result, "https://company.atlassian.net");
  }

  #[test]
  fn test_resolve_jira_base_url_with_scheme_in_env() {
    let guard = EnvVarGuard::new(ENV_JIRA_HOST);
    guard.set("https://company.atlassian.net");

    let result = resolve_jira_base_url().unwrap();
    assert_eq!(result, "https://company.atlassian.net");
  }

  #[test]
  fn test_resolve_jira_base_url_missing_env_var() {
    let guard = EnvVarGuard::new(ENV_JIRA_HOST);
    guard.remove();

    let result = resolve_jira_base_url();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains(ENV_JIRA_HOST));
  }

  // Tests for resolve_github_repo

  #[test]
  fn test_resolve_github_repo_https_remote() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    create_commit(repo, "README.md", "# Test", "Initial commit").unwrap();

    repo.remote("origin", "https://github.com/owner/repo-name.git").unwrap();

    let (owner, repo_name) = resolve_github_repo(git_repo.path()).unwrap();
    assert_eq!(owner, "owner");
    assert_eq!(repo_name, "repo-name");
  }

  #[test]
  fn test_resolve_github_repo_ssh_remote() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    create_commit(repo, "README.md", "# Test", "Initial commit").unwrap();

    repo.remote("origin", "git@github.com:my-org/my-project.git").unwrap();

    let (owner, repo_name) = resolve_github_repo(git_repo.path()).unwrap();
    assert_eq!(owner, "my-org");
    assert_eq!(repo_name, "my-project");
  }

  #[test]
  fn test_resolve_github_repo_no_origin() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    create_commit(repo, "README.md", "# Test", "Initial commit").unwrap();

    let result = resolve_github_repo(git_repo.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("origin"));
  }
}
