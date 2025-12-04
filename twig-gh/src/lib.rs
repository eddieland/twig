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

// Re-export the client
pub use auth::{
  check_github_credentials, create_github_client_from_netrc, create_github_runtime_and_client, get_github_credentials,
};
pub use client::{GitHubClient, create_github_client};
// Re-export models
pub use models::{
  CheckRun, CheckSuite, GitHubAuth, GitHubPullRequest, GitHubUser, PullRequestRef, PullRequestReview, PullRequestStatus,
};
