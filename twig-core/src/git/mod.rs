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
  BranchAnnotationValue, BranchDivergence, BranchEdge, BranchGraph, BranchGraphBuilder, BranchGraphError, BranchHead,
  BranchKind, BranchName, BranchNode, BranchNodeMetadata, BranchStaleState, BranchTopology,
};
pub use renderer::{
  BranchTableColorMode, BranchTableColumn, BranchTableColumnKind, BranchTableRenderError, BranchTableRenderer,
  BranchTableSchema, BranchTableStyle, ORPHAN_BRANCH_ANNOTATION_KEY,
};
pub use repository::{get_repository, get_repository_from_path};
pub use switch::{
  BranchBase, BranchBaseResolution, BranchBaseSource, BranchCreationBase, BranchCreationPolicy, BranchParentReference,
  BranchParentRequest, BranchStateMutations, BranchSwitchAction, BranchSwitchContext, BranchSwitchOutcome,
  BranchSwitchRequest, BranchSwitchService, BranchSwitchTarget, GitHubPullRequestReference, IssueAssociation,
  IssueReference, PullRequestHead, SwitchInput, detect_switch_input, extract_jira_issue_from_url, lookup_branch_tip,
  parse_jira_issue_key, resolve_branch_base, store_github_pr_association, store_jira_association,
  switch_or_create_local_branch, try_checkout_remote_branch,
};

pub use crate::github::{extract_pr_number_from_url, extract_repo_info_from_url};
