//! # Twig Core Library
//!
//! Core library for twig plugins providing configuration structures, state
//! models, and utility functions for plugin developers. This crate enables
//! plugins to access twig's configuration and state in a read-only manner while
//! maintaining their own separate state.

pub mod config;
pub mod creds;
pub mod git;
pub mod jira_parser;
pub mod output;
pub mod state;
pub mod tree_renderer;
pub mod utils;

// Re-export main types for plugin developers
pub use config::{ConfigDirs, get_config_dirs};
pub use creds::{Credentials, netrc, platform};
pub use git::switch::{
  BranchBaseSource, BranchCreationBase, BranchCreationPolicy, BranchParentReference, BranchParentRequest,
  BranchStateMutations, BranchSwitchAction, BranchSwitchContext, BranchSwitchOutcome, BranchSwitchRequest,
  BranchSwitchService, BranchSwitchTarget, GitHubPullRequestReference, JiraIssueReference, PullRequestHead,
};
pub use git::{checkout_branch, current_branch, detect_repository, detect_repository_from_path, in_git_repository};
pub use jira_parser::{JiraParseError, JiraParsingConfig, JiraParsingMode, JiraTicketParser, create_jira_parser};
pub use output::{ColorMode, format_repo_path, print_error, print_info, print_success, print_warning};
pub use state::{
  BranchDependency, BranchMetadata as StateBranchMetadata, Registry, RepoState, Repository, RootBranch, create_worktree,
};
pub use utils::{get_current_branch_github_pr, get_current_branch_jira_issue, open_url_in_browser};

/// Plugin-specific utilities
pub mod plugin {
  use std::path::PathBuf;

  use anyhow::Result;

  pub use super::config::get_config_dirs;
  pub use super::git::{checkout_branch, current_branch};

  /// Get plugin-specific config directory
  pub fn plugin_config_dir(plugin_name: &str) -> Result<PathBuf> {
    let config_dirs = get_config_dirs()?;
    Ok(config_dirs.config_dir().join("plugins").join(plugin_name))
  }

  /// Get plugin-specific data directory
  pub fn plugin_data_dir(plugin_name: &str) -> Result<PathBuf> {
    let config_dirs = get_config_dirs()?;
    Ok(config_dirs.data_dir().join("plugins").join(plugin_name))
  }
}
