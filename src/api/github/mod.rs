mod client;
mod endpoints;
mod models;
mod utils;

// Re-export the client
#[allow(unused_imports)]
pub use client::{GitHubClient, create_github_client};
// Re-export models
#[allow(unused_imports)]
pub use models::{
  GitHubAuth, GitHubCheckRun, GitHubCheckSuite, GitHubPRRef, GitHubPRReview, GitHubPRStatus, GitHubPullRequest,
  GitHubUser,
};
