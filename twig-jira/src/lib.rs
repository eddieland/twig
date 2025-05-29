//! # Jira API Client
//!
//! Provides Jira REST API integration for issue management, transitions, and
//! project data, supporting authentication and common Jira operations for twig
//! workflows.

mod client;
mod endpoints;
pub mod models;

// Re-export the client
pub use client::{JiraClient, create_jira_client};
// Re-export models
pub use models::{
  JiraAuth, JiraIssue, JiraIssueFields, JiraIssueStatus, JiraTransition, JiraTransitions, JiraUser, TransitionId,
  TransitionRequest,
};
