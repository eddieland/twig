//! # GitHub API Client
//!
//! Provides GitHub REST API integration for pull requests, checks, and user
//! data, supporting authentication and common GitHub operations for twig
//! workflows.

mod client;
pub mod endpoints;
pub mod models;
mod utils;

// Re-export the client
pub use client::{GitHubClient, create_github_client};
// Re-export models
pub use models::{
  GitHubAuth, GitHubCheckRun, GitHubCheckSuite, GitHubPRRef, GitHubPRReview, GitHubPRStatus, GitHubPullRequest,
  GitHubUser,
};
