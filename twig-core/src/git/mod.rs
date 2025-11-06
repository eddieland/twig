//! Git utility modules for interacting with repositories and branches.
//!
//! The module is split into focused submodules so consumers can depend on
//! specific areas of git functionality without pulling unrelated helpers.

pub mod branches;
pub mod detection;
pub mod graph;
pub mod repository;

pub use branches::{branch_exists, checkout_branch, current_branch, get_local_branches, get_upstream_branch};
pub use detection::{detect_repository, detect_repository_from_path, in_git_repository};
pub use graph::{
  BranchAnnotationValue, BranchEdge, BranchGraph, BranchGraphBuilder, BranchGraphError, BranchHead, BranchKind,
  BranchName, BranchNode, BranchNodeMetadata, BranchStaleState, BranchTopology,
};
pub use repository::{get_repository, get_repository_from_path};
