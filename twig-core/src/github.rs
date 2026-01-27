//! GitHub URL parsing helpers shared across crates.
//!
//! These helpers intentionally live in `twig-core` so both the CLI and service
//! clients can parse GitHub URLs without depending on a client instance.
//!
//! The parsing approach is inspired by [gix-url](https://docs.rs/gix-url) from
//! the gitoxide project, using structured types and explicit scheme detection
//! rather than ad-hoc string manipulation.

use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

static GITHUB_REPO_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com[/:]([^/]+)/([^/\.]+)").expect("Failed to compile GitHub repo regex"));

static GITHUB_PR_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").expect("Failed to compile GitHub PR regex"));

/// Git remote URL scheme/protocol.
///
/// Used to detect the transport type from a remote URL and make decisions
/// about which URL format to prefer (e.g., SSH vs HTTPS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitRemoteScheme {
  /// SSH protocol (`ssh://` or SCP-style `git@host:path`)
  Ssh,
  /// HTTPS protocol
  Https,
  /// HTTP protocol (insecure)
  Http,
  /// Git protocol (`git://`)
  Git,
  /// Local file path or `file://` URL
  File,
}

impl GitRemoteScheme {
  /// Detect the scheme from a remote URL string.
  ///
  /// Handles standard URL formats as well as SCP-style SSH URLs
  /// (e.g., `git@github.com:owner/repo.git`).
  pub fn detect(url: &str) -> Self {
    if url.starts_with("https://") {
      Self::Https
    } else if url.starts_with("http://") {
      Self::Http
    } else if url.starts_with("git://") {
      Self::Git
    } else if url.starts_with("file://") {
      Self::File
    } else if url.starts_with("ssh://") || Self::is_scp_style(url) {
      Self::Ssh
    } else {
      Self::File
    }
  }

  /// Check if a URL uses SCP-style syntax (user@host:path).
  ///
  /// This detects patterns like `git@github.com:owner/repo.git` which are
  /// SSH URLs without an explicit `ssh://` scheme.
  fn is_scp_style(url: &str) -> bool {
    // Must contain @ before : and : must not be followed by // (which would be a
    // scheme)
    if let Some(at_pos) = url.find('@')
      && let Some(colon_pos) = url[at_pos..].find(':')
    {
      let abs_colon = at_pos + colon_pos;
      // Ensure colon is not part of a scheme (not followed by //)
      let after_colon = &url[abs_colon + 1..];
      return !after_colon.starts_with("//");
    }
    false
  }

  /// Returns true if this scheme uses SSH-based authentication.
  ///
  /// Note: The `git://` protocol is unauthenticated and does NOT use SSH,
  /// so `Git` returns false here.
  pub fn prefers_ssh(&self) -> bool {
    matches!(self, Self::Ssh)
  }
}

/// Parsed GitHub repository reference.
///
/// Represents the owner and repository name extracted from a GitHub URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepo {
  /// Repository owner (user or organization)
  pub owner: String,
  /// Repository name
  pub repo: String,
}

impl GitHubRepo {
  /// Parse a GitHub URL to extract repository information.
  ///
  /// Supports HTTPS, SSH, and URLs containing additional path segments (e.g.,
  /// pull request paths). Returns an error when the URL does not resemble a
  /// GitHub repository path.
  pub fn parse(url: &str) -> Result<Self> {
    if let Some(captures) = GITHUB_REPO_REGEX.captures(url) {
      let owner = captures
        .get(1)
        .expect("capture group 1 must exist")
        .as_str()
        .to_string();
      let repo = captures
        .get(2)
        .expect("capture group 2 must exist")
        .as_str()
        .to_string();
      Ok(Self { owner, repo })
    } else {
      Err(anyhow::anyhow!("Could not extract owner and repo from URL: {url}"))
    }
  }

  /// Returns the full repository path as `owner/repo`.
  pub fn full_name(&self) -> String {
    format!("{}/{}", self.owner, self.repo)
  }
}

/// Parsed GitHub pull request reference.
///
/// Contains the repository information plus the PR number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPr {
  /// Repository owner (user or organization)
  pub owner: String,
  /// Repository name
  pub repo: String,
  /// Pull request number
  pub number: u32,
}

impl GitHubPr {
  /// Parse a GitHub pull request URL.
  ///
  /// Accepts standard pull request URLs and URLs with fragments or query
  /// parameters. Returns an error if the URL does not contain a valid
  /// repository path or PR number.
  pub fn parse(url: &str) -> Result<Self> {
    let repo = GitHubRepo::parse(url)?;

    let number = if let Some(captures) = GITHUB_PR_REGEX.captures(url) {
      let pr_str = captures.get(1).expect("capture group 1 must exist").as_str();
      pr_str
        .parse::<u32>()
        .with_context(|| format!("Failed to parse PR number '{pr_str}' as a valid integer"))?
    } else {
      return Err(anyhow::anyhow!("Could not extract PR number from URL: {url}"));
    };

    Ok(Self {
      owner: repo.owner,
      repo: repo.repo,
      number,
    })
  }

  /// Returns the repository portion of this PR reference.
  pub fn repo(&self) -> GitHubRepo {
    GitHubRepo {
      owner: self.owner.clone(),
      repo: self.repo.clone(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod git_remote_scheme {
    use super::*;

    #[test]
    fn detect_https() {
      assert_eq!(
        GitRemoteScheme::detect("https://github.com/owner/repo"),
        GitRemoteScheme::Https
      );
      assert_eq!(
        GitRemoteScheme::detect("https://github.com/owner/repo.git"),
        GitRemoteScheme::Https
      );
    }

    #[test]
    fn detect_http() {
      assert_eq!(
        GitRemoteScheme::detect("http://github.com/owner/repo"),
        GitRemoteScheme::Http
      );
    }

    #[test]
    fn detect_ssh_explicit() {
      assert_eq!(
        GitRemoteScheme::detect("ssh://git@github.com/owner/repo"),
        GitRemoteScheme::Ssh
      );
    }

    #[test]
    fn detect_ssh_scp_style() {
      assert_eq!(
        GitRemoteScheme::detect("git@github.com:owner/repo.git"),
        GitRemoteScheme::Ssh
      );
      assert_eq!(GitRemoteScheme::detect("user@host:path/to/repo"), GitRemoteScheme::Ssh);
    }

    #[test]
    fn detect_git_protocol() {
      assert_eq!(
        GitRemoteScheme::detect("git://github.com/owner/repo"),
        GitRemoteScheme::Git
      );
    }

    #[test]
    fn detect_file() {
      assert_eq!(GitRemoteScheme::detect("file:///path/to/repo"), GitRemoteScheme::File);
      assert_eq!(GitRemoteScheme::detect("/path/to/repo"), GitRemoteScheme::File);
      assert_eq!(GitRemoteScheme::detect("../relative/path"), GitRemoteScheme::File);
    }

    #[test]
    fn prefers_ssh() {
      assert!(GitRemoteScheme::Ssh.prefers_ssh());
      // git:// is unauthenticated, not SSH
      assert!(!GitRemoteScheme::Git.prefers_ssh());
      assert!(!GitRemoteScheme::Https.prefers_ssh());
      assert!(!GitRemoteScheme::Http.prefers_ssh());
      assert!(!GitRemoteScheme::File.prefers_ssh());
    }
  }

  mod github_repo {
    use super::*;

    #[test]
    fn parse_https() {
      let repo = GitHubRepo::parse("https://github.com/omenien/twig").unwrap();
      assert_eq!(repo.owner, "omenien");
      assert_eq!(repo.repo, "twig");
    }

    #[test]
    fn parse_git_suffix_and_trailing_slash() {
      let repo = GitHubRepo::parse("https://github.com/omenien/twig.git/").unwrap();
      assert_eq!(repo.owner, "omenien");
      assert_eq!(repo.repo, "twig");
    }

    #[test]
    fn parse_with_path() {
      let repo = GitHubRepo::parse("https://github.com/omenien/twig/pull/123").unwrap();
      assert_eq!(repo.owner, "omenien");
      assert_eq!(repo.repo, "twig");
    }

    #[test]
    fn parse_ssh() {
      let repo = GitHubRepo::parse("git@github.com:omenien/twig.git").unwrap();
      assert_eq!(repo.owner, "omenien");
      assert_eq!(repo.repo, "twig");
    }

    #[test]
    fn parse_invalid() {
      assert!(GitHubRepo::parse("https://example.com/not-github").is_err());
      assert!(GitHubRepo::parse("https://github.com/only-owner").is_err());
    }

    #[test]
    fn full_name() {
      let repo = GitHubRepo::parse("https://github.com/omenien/twig").unwrap();
      assert_eq!(repo.full_name(), "omenien/twig");
    }
  }

  mod github_pr {
    use super::*;

    #[test]
    fn parse_valid() {
      let pr = GitHubPr::parse("https://github.com/omenien/twig/pull/123").unwrap();
      assert_eq!(pr.owner, "omenien");
      assert_eq!(pr.repo, "twig");
      assert_eq!(pr.number, 123);
    }

    #[test]
    fn parse_with_fragment_and_query() {
      let pr = GitHubPr::parse("https://github.com/omenien/twig/pull/456#discussion_r123456789").unwrap();
      assert_eq!(pr.number, 456);

      let pr = GitHubPr::parse("https://github.com/omenien/twig/pull/456?utm_source=test").unwrap();
      assert_eq!(pr.number, 456);
    }

    #[test]
    fn parse_invalid() {
      assert!(GitHubPr::parse("https://github.com/omenien/twig").is_err());
      assert!(GitHubPr::parse("https://github.com/omenien/twig/pull/abc").is_err());
    }

    #[test]
    fn repo_accessor() {
      let pr = GitHubPr::parse("https://github.com/omenien/twig/pull/123").unwrap();
      let repo = pr.repo();
      assert_eq!(repo.owner, "omenien");
      assert_eq!(repo.repo, "twig");
    }
  }
}
