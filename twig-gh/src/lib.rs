//! # GitHub API Client
//!
//! Provides GitHub REST API integration for pull requests, checks, and user
//! data, supporting authentication and common GitHub operations for twig
//! workflows.

pub mod client;
pub mod consts;
pub mod endpoints;
pub mod models;
pub mod utils;

// Re-export the client
pub use client::{GitHubClient, create_github_client};
// Re-export models
pub use models::{
  CheckRun, CheckSuite, GitHubAuth, GitHubIssue, GitHubLabel, GitHubPullRequest, GitHubUser, PullRequestRef,
  PullRequestReview, PullRequestStatus,
};
// Re-export endpoints structs
pub use endpoints::pulls::CreatePullRequestParams;
