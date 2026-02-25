//! Git utility modules for interacting with repositories and branches.
//!
//! The module is split into focused submodules so consumers can depend on
//! specific areas of git functionality without pulling unrelated helpers.

use anyhow::{Context, Result};
use git2::{Oid, Repository as Git2Repository};

pub mod branches;
pub mod detection;
pub mod graph;
pub mod renderer;
pub mod repository;
pub mod switch;
pub mod tree;

pub use branches::{branch_exists, checkout_branch, current_branch, get_local_branches, get_upstream_branch};
pub use detection::{detect_repository, detect_repository_from_path, in_git_repository, resolve_to_main_repo_path};
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
  IssueReference, ParentBranchOption, PullRequestCheckoutOutcome, PullRequestCheckoutRequest, PullRequestHead,
  PullRequestHeadInfo, SwitchExecutionOptions, SwitchInput, checkout_pr_branch, detect_switch_input,
  extract_jira_issue_from_url, fetch_remote_branch, lookup_branch_tip, parse_jira_issue_key, resolve_branch_base,
  resolve_pr_remote, sanitize_remote_name, select_repo_url, store_github_pr_association, store_jira_association,
  switch_or_create_local_branch, try_checkout_remote_branch,
};
pub use tree::{
  annotate_orphaned_branches, attach_orphans_to_default_root, default_root_branch, determine_render_root,
  filter_branch_graph, find_orphaned_branches,
};

pub use crate::github::{GitHubPr, GitHubRepo, GitRemoteScheme};

/// Get commits ahead/behind between two branches within the provided
/// repository.
///
/// Uses `git2::Repository::graph_ahead_behind` to compute how many commits
/// `branch` is ahead of and behind `base`.
///
/// # Errors
/// Returns an error if either branch cannot be resolved or if the computation
/// fails.
pub fn get_commits_ahead_behind(repo: &Git2Repository, branch: &str, base: &str) -> Result<(usize, usize)> {
  let branch_oid =
    resolve_commit_oid(repo, branch).with_context(|| format!("Failed to resolve branch '{branch}'"))?;
  let base_oid = resolve_commit_oid(repo, base).with_context(|| format!("Failed to resolve branch '{base}'"))?;

  let (ahead, behind) = repo
    .graph_ahead_behind(branch_oid, base_oid)
    .with_context(|| format!("Unable to compute ahead/behind between '{branch}' and '{base}'"))?;

  Ok((ahead, behind))
}

/// Resolve a branch name (or ref-like string) to a commit OID.
fn resolve_commit_oid(repo: &Git2Repository, name: &str) -> Result<Oid> {
  // Prefer the local branch ref if it exists
  if let Ok(reference) = repo.find_reference(&format!("refs/heads/{name}")) {
    if let Some(target) = reference.target() {
      return Ok(target);
    }
  }

  // Fall back to revparse
  let object = repo
    .revparse_single(name)
    .with_context(|| format!("Unable to resolve reference '{name}'"))?;
  let commit = object
    .peel_to_commit()
    .with_context(|| format!("Reference '{name}' does not point to a commit"))?;
  Ok(commit.id())
}

#[cfg(test)]
mod tests {
  use std::fs;
  use std::path::Path;

  use git2::Repository;
  use git2::build::CheckoutBuilder;
  use tempfile::TempDir;

  use super::*;

  fn configure_identity(repo: &Repository) {
    let mut config = repo.config().expect("config");
    config.set_str("user.name", "Twig Bot").expect("set user.name");
    config.set_str("user.email", "twig@example.com").expect("set user.email");
  }

  fn checkout_branch(repo: &Repository, name: &str) {
    repo.set_head(&format!("refs/heads/{name}")).expect("set HEAD");
    repo
      .checkout_head(Some(CheckoutBuilder::default().force()))
      .expect("checkout");
  }

  fn commit_file(repo: &Repository, path: &Path, contents: &str, message: &str) {
    fs::write(path, contents).expect("write file");
    let mut index = repo.index().expect("index");
    let rel_path = path.strip_prefix(repo.workdir().expect("workdir")).expect("strip prefix");
    index.add_path(rel_path).expect("add path");
    index.write().expect("write index");

    let tree_id = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_id).expect("find tree");
    let signature = git2::Signature::now("Twig Bot", "twig@example.com").expect("signature");

    let parent_commit = repo
      .head()
      .ok()
      .and_then(|h| h.target())
      .and_then(|oid| repo.find_commit(oid).ok());
    match parent_commit {
      Some(parent) => {
        repo
          .commit(Some("HEAD"), &signature, &signature, message, &tree, &[&parent])
          .expect("commit with parent");
      }
      None => {
        repo
          .commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
          .expect("initial commit");
      }
    }
  }

  #[test]
  fn computes_ahead_and_behind_counts() {
    let temp = TempDir::new().expect("temp dir");
    let repo = Repository::init(temp.path()).expect("init repo");
    configure_identity(&repo);

    let workdir = repo.workdir().expect("workdir").to_path_buf();

    // initial commit on default branch
    let file_path = workdir.join("file.txt");
    commit_file(&repo, &file_path, "initial", "initial");

    // create main branch and switch to it
    let head_commit = repo.head().expect("head").peel_to_commit().expect("peel");
    repo.branch("main", &head_commit, true).expect("create main");
    checkout_branch(&repo, "main");

    // main commit 2
    commit_file(&repo, &file_path, "main-one", "main-one");

    // create feature branch from current main
    let current_commit = repo.head().expect("head").peel_to_commit().expect("peel");
    repo.branch("feature/work", &current_commit, true).expect("create feature");
    checkout_branch(&repo, "feature/work");

    // feature commit — makes branch 1 ahead
    commit_file(&repo, &file_path, "feature-change", "feature-change");

    // switch back to main and add another commit — makes feature 1 behind
    checkout_branch(&repo, "main");
    commit_file(&repo, &file_path, "main-two", "main-two");

    let (ahead, behind) = get_commits_ahead_behind(&repo, "feature/work", "main").expect("ahead/behind");
    assert_eq!(ahead, 1);
    assert_eq!(behind, 1);
  }
}
