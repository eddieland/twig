//! Shared branch switching interface used by the CLI and Twig plugins.
//!
//! This module defines the request/response types and planning primitives for
//! future branch switching services. The goal is to centralise behaviour that
//! currently lives inside `twig-cli` so that the forthcoming `twig flow`
//! plugin can rely on the same implementation without duplicating logic or
//! tightly coupling to CLI-specific messaging.

use std::path::Path;

use anyhow::{Context, Result};
use git2::{BranchType, Oid, Repository};

use crate::git::checkout_branch;
use crate::git::graph::BranchName;
use crate::state::RepoState;

/// Switch to the provided branch, creating it from `HEAD` when missing.
///
/// This helper mirrors the simple switching behaviour previously embedded in
/// the `twig flow` plugin so that CLI and plugin callers can share logic.
/// Callers are responsible for ensuring the repository is in a usable state
/// (non-bare, working tree present).
pub fn switch_or_create_local_branch(repository: &Repository, target: &BranchName) -> Result<BranchSwitchOutcome> {
  if branch_exists(repository, target) {
    checkout_branch(repository, target.as_str())?;

    return Ok(BranchSwitchOutcome {
      branch: target.clone(),
      action: BranchSwitchAction::CheckedOutExisting,
      state_mutations: BranchStateMutations::default(),
    });
  }

  let head_commit = repository
    .head()
    .context("Repository does not have an active HEAD commit")?
    .peel_to_commit()
    .context("Failed to resolve HEAD commit")?;

  repository
    .branch(target.as_str(), &head_commit, false)
    .with_context(|| format!("Failed to create branch \"{target}\" from HEAD"))?;
  checkout_branch(repository, target.as_str())?;

  Ok(BranchSwitchOutcome {
    branch: target.clone(),
    action: BranchSwitchAction::Created {
      base: BranchCreationBase {
        commit: head_commit.id(),
        source: BranchBaseSource::Head,
      },
      upstream: None,
    },
    state_mutations: BranchStateMutations::default(),
  })
}

fn branch_exists(repository: &Repository, target: &BranchName) -> bool {
  repository.find_branch(target.as_str(), BranchType::Local).is_ok()
}

/// Request describing a branch switch operation.
///
/// Callers construct this request after parsing CLI arguments or other user
/// input. Concrete services can then translate the high-level intent into a
/// sequence of Git operations and repository state updates.
#[derive(Debug, Clone)]
pub struct BranchSwitchRequest {
  /// Target that the user wants to end up on.
  pub target: BranchSwitchTarget,
  /// Whether the service may create new branches when the target is missing.
  pub creation_policy: BranchCreationPolicy,
  /// Preference for which branch (or commit) should act as the parent/base when
  /// creating a new branch.
  pub parent: BranchParentRequest,
  /// When true, the service should plan actions without mutating the
  /// repository.
  pub dry_run: bool,
}

impl BranchSwitchRequest {
  /// Convenience constructor for branch-name based requests that allow branch
  /// creation when the target does not already exist.
  pub fn for_branch(name: impl Into<BranchName>) -> Self {
    Self {
      target: BranchSwitchTarget::Branch(name.into()),
      creation_policy: BranchCreationPolicy::AllowCreate,
      parent: BranchParentRequest::Default,
      dry_run: false,
    }
  }
}

/// Target for a branch switch request.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchSwitchTarget {
  /// Switch to the root of the current branch's dependency tree.
  DependencyRoot,
  /// Switch to an explicit branch name.
  Branch(BranchName),
  /// Switch to (or create) a branch associated with an external issue (Jira,
  /// etc.).
  Issue(IssueReference),
  /// Switch to (or create) a branch associated with a GitHub pull request.
  GitHubPullRequest(GitHubPullRequestReference),
}

/// Reference to an external issue/work item used when creating or locating
/// associated branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueReference {
  /// Issue identifier (`PROJ-123`, `#42`, etc.).
  pub key: String,
  /// Optional human readable summary that can be used when generating branch
  /// names.
  pub summary: Option<String>,
}

impl IssueReference {
  /// Construct a reference for the provided tracker.
  pub fn new(key: impl Into<String>) -> Self {
    Self {
      key: key.into(),
      summary: None,
    }
  }

  /// Convenience helper for Jira references.
  pub fn jira(key: impl Into<String>) -> Self {
    Self::new(key)
  }
}

/// Reference to a GitHub pull request used when creating or locating branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestReference {
  /// Owning user or organisation for the repository.
  pub owner: Option<String>,
  /// Repository name.
  pub repository: Option<String>,
  /// Pull request sequence number.
  pub number: u32,
  /// Optional head information when already known (e.g. from a previous query).
  pub head: Option<PullRequestHead>,
}

impl GitHubPullRequestReference {
  /// Construct a GitHub pull request reference with minimal information.
  pub fn new(number: u32) -> Self {
    Self {
      owner: None,
      repository: None,
      number,
      head: None,
    }
  }
}

/// Description of a pull request head reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestHead {
  /// Branch name advertised by the pull request head.
  pub branch: BranchName,
  /// Optional remote name if it should be tracked explicitly.
  pub remote: Option<String>,
}

/// Policy controlling whether a missing branch may be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchCreationPolicy {
  /// Only allow switching when the branch already exists locally.
  RequireExisting,
  /// Permit the service to create the branch when it cannot be found.
  #[default]
  AllowCreate,
}

/// Preference for resolving the parent/base branch when creating a new branch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BranchParentRequest {
  /// Use the repository's current HEAD commit without linking dependencies.
  #[default]
  Default,
  /// Use the currently checked-out branch as the parent.
  CurrentBranch,
  /// Use an explicitly named branch.
  Explicit(BranchName),
  /// Resolve the parent branch by looking up the provided issue key.
  IssueKey(String),
  /// Do not link the new branch to any parent.
  None,
}

/// Environment required to plan a branch switch.
pub struct BranchSwitchContext<'repo> {
  /// Backing git2 repository handle.
  pub repository: &'repo Repository,
  /// Filesystem path to the repository root.
  pub repository_path: &'repo Path,
  /// Snapshot of Twig's repository state metadata.
  pub repo_state: &'repo RepoState,
}

/// Concrete actions that a branch switch service may perform.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchSwitchAction {
  /// The requested branch was already checked out.
  AlreadyCurrent,
  /// The branch existed and was checked out locally.
  CheckedOutExisting,
  /// The branch was fetched from a remote and checked out.
  CheckedOutRemote {
    /// Remote that supplied the branch contents.
    remote: String,
    /// Remote reference that was checked out (e.g. `origin/feature/foo`).
    remote_ref: BranchName,
  },
  /// A new local branch was created from the provided base.
  Created {
    /// Description of the base commit and branch used for the new branch.
    base: BranchCreationBase,
    /// Remote tracking branch configured for the new branch, when applicable.
    upstream: Option<String>,
  },
}

/// Description of the base commit used when creating or updating a branch.
#[derive(Debug, Clone)]
pub struct BranchCreationBase {
  /// Commit identifier used for the new branch tip.
  pub commit: Oid,
  /// Source reference that produced the commit.
  pub source: BranchBaseSource,
}

/// Source of a branch base resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BranchBaseSource {
  /// The repository's HEAD commit.
  Head,
  /// An existing local branch.
  LocalBranch(BranchName),
  /// A remote tracking branch.
  RemoteBranch {
    /// Remote name (`origin`, `upstream`, etc.).
    remote: String,
    /// Branch name advertised by the remote.
    branch: BranchName,
  },
}

/// Planned metadata updates that accompany a branch switch.
#[derive(Debug, Clone, Default)]
pub struct BranchStateMutations {
  /// Desired dependency relationship for the branch.
  pub dependency: Option<BranchDependencyUpdate>,
  /// Issue key to associate with the branch.
  pub issue: Option<IssueAssociation>,
  /// GitHub pull request number to associate with the branch.
  pub github_pr: Option<u32>,
}

impl BranchStateMutations {
  /// Determine whether the mutation set is empty.
  pub fn is_empty(&self) -> bool {
    self.dependency.is_none() && self.issue.is_none() && self.github_pr.is_none()
  }
}

/// Association between a branch and an external issue tracker item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueAssociation {
  /// Identifier within the tracker (currently Jira).
  pub key: String,
}

/// Desired dependency relationship for a branch.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchDependencyUpdate {
  /// Remove any existing dependency metadata.
  Clear,
  /// Set the dependency parent to the provided branch reference.
  Set(BranchParentReference),
}

/// Reference to a parent branch used when updating dependency metadata.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchParentReference {
  /// Dependency pointing at an explicit branch name.
  Branch(BranchName),
  /// Dependency resolved via an issue tracker key.
  IssueKey(String),
}

/// Result returned by a branch switch service.
#[derive(Debug, Clone)]
pub struct BranchSwitchOutcome {
  /// Branch name that was ultimately checked out.
  pub branch: BranchName,
  /// Summary of the action that was taken.
  pub action: BranchSwitchAction,
  /// Metadata updates that should be applied after the switch completes.
  pub state_mutations: BranchStateMutations,
}

/// Trait implemented by branch switch planners.
///
/// The planner is responsible for translating high-level requests into a
/// concrete [`BranchSwitchOutcome`]. Implementations are expected to perform
/// necessary Git lookups and return rich metadata so that callers can present
/// consistent messaging and apply repository state updates.
pub trait BranchSwitchService {
  /// Plan a branch switch and return the resulting outcome.
  fn plan_switch(
    &mut self,
    context: BranchSwitchContext<'_>,
    request: BranchSwitchRequest,
  ) -> anyhow::Result<BranchSwitchOutcome>;
}

#[cfg(test)]
mod tests {
  use twig_test_utils::git::{GitRepoTestGuard, create_branch, create_commit};

  use super::*;

  #[test]
  fn checks_out_existing_branch() -> Result<()> {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;
    create_branch(&guard.repo, "feature/existing", None)?;

    let outcome = switch_or_create_local_branch(&guard.repo, &BranchName::from("feature/existing"))?;

    assert!(matches!(outcome.action, BranchSwitchAction::CheckedOutExisting));

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/existing"));

    Ok(())
  }

  #[test]
  fn creates_branch_from_head_when_missing() -> Result<()> {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;

    let outcome = switch_or_create_local_branch(&guard.repo, &BranchName::from("feature/new"))?;

    match outcome.action {
      BranchSwitchAction::Created { base, upstream } => {
        assert_eq!(base.source, BranchBaseSource::Head);
        assert!(upstream.is_none());
      }
      action => panic!("unexpected action {action:?}"),
    }

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/new"));

    Ok(())
  }
}
