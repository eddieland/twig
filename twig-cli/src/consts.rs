//! Constants for the Twig CLI
//!
//! This module defines various constants used throughout the Twig CLI
//! application, including environment variable names, default values, and other
//! static strings.

/// Environment variable for the JIRA host URL
pub const ENV_JIRA_HOST: &str = "JIRA_HOST";

/// Default JIRA host URL if $JIRA_HOST is undefined
pub const DEFAULT_JIRA_HOST: &str = "https://eddieland.atlassian.net";
