//! Constants for the twig-jira client.

/// User-Agent header value for the Jira API client
pub const USER_AGENT: &str = concat!("twig-cli/", env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
