//! Shared server context available to all tool handlers.

use std::path::{Path, PathBuf};

use anyhow::Context;
use twig_core::config::ConfigDirs;
use twig_core::state::RepoState;
use twig_gh::GitHubClient;
use twig_jira::JiraClient;

use crate::types::ToolError;

/// Shared context available to all tool handlers.
pub struct ServerContext {
  pub config_dirs: ConfigDirs,
  pub repo_path: Option<PathBuf>,
  pub home_dir: PathBuf,

  /// Lazily initialized on first GitHub call.
  github_client: tokio::sync::OnceCell<Option<GitHubClient>>,
  /// Lazily initialized on first Jira call. Tuple of (client, host).
  jira_client: tokio::sync::OnceCell<Option<(JiraClient, String)>>,
}

impl ServerContext {
  pub fn new(config_dirs: ConfigDirs, repo_path: Option<PathBuf>, home_dir: PathBuf) -> Self {
    Self {
      config_dirs,
      repo_path,
      home_dir,
      github_client: tokio::sync::OnceCell::new(),
      jira_client: tokio::sync::OnceCell::new(),
    }
  }

  /// Returns the repo path or a structured `ToolError`.
  pub fn require_repo(&self) -> Result<&Path, ToolError> {
    self.repo_path.as_deref().ok_or_else(|| ToolError {
      code: "no_repo".into(),
      message: "twig-mcp was started outside a git repository".into(),
      hint: Some("Run twig-mcp from within a git repository.".into()),
    })
  }

  /// Load fresh `RepoState` from disk.
  pub fn load_repo_state(&self) -> anyhow::Result<RepoState> {
    let repo_path = self.repo_path.as_deref().context("No repository detected")?;
    RepoState::load(repo_path)
  }

  /// Load repo state or return a structured `ToolError`.
  pub fn require_repo_state(&self) -> Result<RepoState, ToolError> {
    let repo_path = self.require_repo()?;
    RepoState::load(repo_path).map_err(|e| ToolError {
      code: "no_twig_state".into(),
      message: format!("Failed to load twig state: {e}"),
      hint: Some("Run `twig init` in this repository first.".into()),
    })
  }

  /// Lazily initialise and return the GitHub client.
  pub async fn get_github_client(&self) -> Result<&GitHubClient, ToolError> {
    let maybe_client = self
      .github_client
      .get_or_init(|| async { twig_gh::create_github_client_from_netrc(&self.home_dir).ok() })
      .await;

    maybe_client.as_ref().ok_or_else(|| ToolError {
      code: "credentials_missing".into(),
      message: "GitHub credentials not found".into(),
      hint: Some("Add credentials for github.com to `~/.netrc`. See `twig auth --help`.".into()),
    })
  }

  /// Lazily initialise and return the Jira client.
  pub async fn get_jira_client(&self) -> Result<&JiraClient, ToolError> {
    let maybe_client = self
      .jira_client
      .get_or_init(|| async {
        let host = twig_jira::get_jira_host().ok()?;
        let client = twig_jira::create_jira_client_from_netrc(&self.home_dir, &host).ok()?;
        Some((client, host))
      })
      .await;

    maybe_client
      .as_ref()
      .map(|(client, _)| client)
      .ok_or_else(|| ToolError {
        code: "credentials_missing".into(),
        message: "Jira credentials not found".into(),
        hint: Some("Set $JIRA_HOST and add credentials to `~/.netrc`. See `twig auth --help`.".into()),
      })
  }

  /// Extract GitHub owner/repo from the git remote URL.
  pub fn get_github_repo(&self) -> Result<twig_core::GitHubRepo, ToolError> {
    let repo_path = self.require_repo()?;
    let repo = git2::Repository::open(repo_path).map_err(|e| ToolError {
      code: "no_repo".into(),
      message: format!("Failed to open git repository: {e}"),
      hint: None,
    })?;
    let remote = repo.find_remote("origin").map_err(|e| ToolError {
      code: "not_found".into(),
      message: format!("No 'origin' remote found: {e}"),
      hint: Some("Add a GitHub remote named 'origin'.".into()),
    })?;
    let url = remote.url().ok_or_else(|| ToolError {
      code: "not_found".into(),
      message: "Remote 'origin' has no URL".into(),
      hint: None,
    })?;
    twig_core::GitHubRepo::parse(url).map_err(|e| ToolError {
      code: "not_found".into(),
      message: format!("Could not extract GitHub repo from remote URL: {e}"),
      hint: Some("Ensure the 'origin' remote points to a GitHub repository.".into()),
    })
  }
}
