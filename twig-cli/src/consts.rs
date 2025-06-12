//! Constants for the Twig CLI
//!
//! This module defines various constants used throughout the Twig CLI
//! application, including environment variable names, default values, and other
//! static strings.

/// Environment variable for the JIRA host URL
pub const ENV_JIRA_HOST: &str = "JIRA_HOST";

/// Platform-specific Git executable name
#[cfg(windows)]
#[cfg_attr(not(windows), allow(dead_code))]
pub const GIT_EXECUTABLE: &str = "git.exe";

/// Platform-specific Git executable name
#[cfg(not(windows))]
#[cfg_attr(windows, allow(dead_code))]
pub const GIT_EXECUTABLE: &str = "git";
