//! Git utility modules for interacting with repositories and branches.
//!
//! The module is split into focused submodules so consumers can depend on
//! specific areas of git functionality without pulling unrelated helpers.

pub mod branches;
pub mod detection;
pub mod graph;
pub mod renderer;
pub mod repository;
pub mod switch;

pub use branches::{branch_exists, checkout_branch, current_branch, get_local_branches, get_upstream_branch};
pub use detection::{detect_repository, detect_repository_from_path, in_git_repository};
pub use graph::{
  BranchAnnotationValue, BranchEdge, BranchGraph, BranchGraphBuilder, BranchGraphError, BranchHead, BranchKind,
  BranchName, BranchNode, BranchNodeMetadata, BranchStaleState, BranchTopology,
};
pub use renderer::{
  BranchTableColumn, BranchTableColumnKind, BranchTableRenderError, BranchTableRenderer, BranchTableSchema,
};
pub use repository::{get_repository, get_repository_from_path};
pub use switch::{
  BranchBaseSource, BranchCreationBase, BranchCreationPolicy, BranchParentReference, BranchParentRequest,
  BranchStateMutations, BranchSwitchAction, BranchSwitchContext, BranchSwitchOutcome, BranchSwitchRequest,
  BranchSwitchService, BranchSwitchTarget, GitHubPullRequestReference, IssueAssociation, IssueReference,
  PullRequestHead, switch_or_create_local_branch,
};

/// Get commits ahead/behind between two branches
///
/// # Arguments
/// * `branch` - The branch to compare
/// * `base` - The base branch to compare against
///
/// # Returns
/// A tuple of (ahead, behind) commit counts
pub fn get_commits_ahead_behind(_branch: &str, _base: &str) -> anyhow::Result<(usize, usize)> {
  // TODO: Implement actual git ahead/behind calculation
  Ok((0, 0))
}
