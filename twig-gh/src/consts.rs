//! Constants for the twig-gh client

/// Base URL for the official SaaS GitHub API
pub const API_BASE_URL: &str = "https://api.github.com";

/// User-Agent header value for the GitHub API client
pub const USER_AGENT: &str = concat!("twig-cli/", env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Accept header value for the GitHub API
pub const ACCEPT: &str = "application/vnd.github.v3+json";
