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
pub mod tree;

pub use branches::{branch_exists, checkout_branch, current_branch, get_local_branches, get_upstream_branch};
pub use detection::{detect_repository, detect_repository_from_path, in_git_repository};
pub use graph::{
  BranchAnnotationValue, BranchDivergence, BranchEdge, BranchGraph, BranchGraphBuilder, BranchGraphError, BranchHead,
  BranchKind, BranchName, BranchNode, BranchNodeMetadata, BranchStaleState, BranchTopology,
};
pub use renderer::{
  BranchTableColorMode, BranchTableColumn, BranchTableColumnKind, BranchTableLinkMode, BranchTableLinks,
  BranchTableRenderError, BranchTableRenderer, BranchTableSchema, BranchTableStyle, ORPHAN_BRANCH_ANNOTATION_KEY,
};
pub use repository::{get_repository, get_repository_from_path};
pub use switch::{
  BranchBase, BranchBaseResolution, BranchBaseSource, BranchCreationBase, BranchCreationPolicy, BranchParentReference,
  BranchParentRequest, BranchStateMutations, BranchSwitchAction, BranchSwitchContext, BranchSwitchOutcome,
  BranchSwitchRequest, BranchSwitchService, BranchSwitchTarget, GitHubPullRequestReference, IssueAssociation,
  IssueReference, PullRequestHead, SwitchInput, detect_switch_input, extract_github_repo_from_url,
  extract_jira_issue_from_url, extract_pr_number_from_url, lookup_branch_tip, parse_jira_issue_key,
  resolve_branch_base, store_github_pr_association, store_jira_association, switch_or_create_local_branch,
  try_checkout_remote_branch,
};
pub use tree::{
  BranchSelection, annotate_orphaned_branches, attach_orphans_to_default_root, build_branch_table_links,
  current_branch_name, default_root_branch, determine_render_root, display_orphan_note, filter_branch_graph,
  find_orphaned_branches, handle_graph_error, load_repo_state, render_branch_table, resolve_github_repo,
  resolve_jira_base_url, select_parent_branch, select_root_branch,
};

/// Get commits ahead/behind between two branches.
pub fn get_commits_ahead_behind(_branch: &str, _base: &str) -> anyhow::Result<(usize, usize)> {
  // TODO: Implement actual git ahead/behind calculation.
  Ok((0, 0))
}
