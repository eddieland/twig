//! # GitHub API Client
//!
//! Provides GitHub REST API integration for pull requests, checks, and user
//! data, supporting authentication and common GitHub operations for twig
//! workflows.

pub mod auth;
pub mod client;
pub mod consts;
pub mod endpoints;
pub mod models;
pub mod utils;

// Re-export the client
pub use auth::{
  check_github_credentials, create_github_client_from_netrc, create_github_runtime_and_client, get_github_credentials,
};
pub use client::{GitHubClient, create_github_client};
// Re-export models
pub use models::{
  CheckRun, CheckSuite, GitHubAuth, GitHubPullRequest, GitHubUser, PullRequestRef, PullRequestReview, PullRequestStatus,
};
// Re-export utilities
pub use utils::{extract_pr_number_from_url, extract_repo_info_from_url};
