//! # Twig Core Library
//!
//! Core library for twig plugins providing configuration structures, state
//! models, and utility functions for plugin developers. This crate enables
//! plugins to access twig's configuration and state in a read-only manner while
//! maintaining their own separate state.

pub mod complete;
pub mod config;
pub mod creds;
pub mod git;
pub mod github;
pub mod jira_parser;
pub mod output;
pub mod plugin;
pub mod prompts;
pub mod state;
pub mod text;
pub mod tree_renderer;
pub mod url;
pub mod utils;

// Re-export main types for plugin developers
pub use config::{ConfigDirs, get_config_dirs};
pub use creds::{Credentials, netrc, platform};
pub use git::switch::{
  BranchBase, BranchBaseResolution, BranchBaseSource, BranchCreationBase, BranchCreationPolicy, BranchParentReference,
  BranchParentRequest, BranchStateMutations, BranchSwitchAction, BranchSwitchContext, BranchSwitchOutcome,
  BranchSwitchRequest, BranchSwitchService, BranchSwitchTarget, GitHubPullRequestReference, IssueAssociation,
  IssueReference, PullRequestCheckoutOutcome, PullRequestCheckoutRequest, PullRequestHead, PullRequestHeadInfo,
  SwitchInput, checkout_pr_branch, detect_switch_input, extract_jira_issue_from_url, fetch_remote_branch,
  lookup_branch_tip, parse_jira_issue_key, resolve_branch_base, resolve_pr_remote, sanitize_remote_name,
  select_repo_url, store_github_pr_association, store_jira_association, try_checkout_remote_branch,
};
pub use git::{
  checkout_branch, current_branch, detect_repository, detect_repository_from_path, get_repository, in_git_repository,
};
pub use github::{GitHubPr, GitHubRepo, GitRemoteScheme};
pub use jira_parser::{JiraParseError, JiraParsingConfig, JiraParsingMode, JiraTicketParser, create_jira_parser};
pub use output::{ColorMode, cli_styles, format_repo_path, print_error, print_info, print_success, print_warning};
pub use plugin::{PluginContext, plugin_config_dir, plugin_data_dir};
pub use prompts::twig_theme;
pub use state::{
  BranchDependency, BranchMetadata as StateBranchMetadata, Registry, RepoState, Repository, RootBranch, create_worktree,
};
pub use text::{Hyperlink, HyperlinkExt, hyperlink, hyperlinks_disabled, set_hyperlinks_override, truncate_string};
pub use url::{
  ENV_JIRA_HOST, ensure_url_scheme, resolve_github_repo, resolve_github_repo_from_git2, resolve_jira_base_url,
};
pub use utils::{
  filter_stop_words, generate_branch_name_from_issue, get_current_branch_github_pr, get_current_branch_jira_issue,
  open_url_in_browser,
};
