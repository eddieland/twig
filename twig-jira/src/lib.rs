//! # Jira API Client
//!
//! Provides Jira REST API integration for issue management, transitions, and
//! project data, supporting authentication and common Jira operations for twig
//! workflows.

pub mod auth;
pub mod client;
pub mod consts;
pub mod endpoints;
pub mod models;

// Re-export the client
pub use auth::{
  ENV_JIRA_HOST, check_jira_credentials, create_jira_client_from_netrc, create_jira_runtime_and_client,
  get_jira_credentials, get_jira_host,
};
pub use client::{JiraClient, create_jira_client};
// Re-export models
pub use models::{
  Issue, IssueFields, IssueStatus, JiraAuth, JiraUser, Transition, TransitionId, TransitionRequest, Transitions,
};
