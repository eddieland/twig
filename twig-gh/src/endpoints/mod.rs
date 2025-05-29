//! # GitHub API Endpoints
//!
//! Organized endpoint implementations for different GitHub API resource types,
//! including pulls, checks, and user management functionality.

pub mod checks;
pub mod pulls;
pub mod users;

// Tests have been implemented for the following GitHub endpoints:
// - list_pull_requests: Test request formatting and response parsing
// - get_pull_request: Test request with PR number and response parsing
// - get_pr_status: Test combined status request and response
// - get_check_runs: Test request formatting and response parsing
// - find_pull_requests_by_head_branch: Test filtering PRs by head branch
//
// TODO: Add tests for the following GitHub endpoints:
// - get_user: Test request formatting and response parsing

#[cfg(test)]
mod tests {
  // Tests will be implemented here
}
