//! Branch graph domain models shared between the CLI and `twig` plugins.
//!
//! The goal of this module is to provide a reusable representation of a Git
//! branch tree that can be consumed by renderers (textual trees, tabular
//! summaries, etc.) as well as higher-level workflows like branch switching.
//! The data structures focus on separating the *topology* of a repository from
//! contextual metadata so that additional annotations can be layered on without
//! duplicating graph construction.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use anyhow::Error as AnyError;
use chrono::{DateTime, Utc};
use git2::Oid;
use thiserror::Error;

/// Canonical identifier for a branch within a [`BranchGraph`].
///
/// The identifier wraps an `Arc<str>` to make it cheap to clone while keeping
/// the type distinct from arbitrary strings. Consumers should prefer calling
/// [`BranchName::as_str`] when they need to display or compare values.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BranchName(Arc<str>);

impl BranchName {
  /// Construct a branch name reference from any string-like value.
  pub fn new(name: impl Into<Arc<str>>) -> Self {
    Self(name.into())
  }

  /// Borrow the underlying branch name as a `&str`.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for BranchName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("BranchName").field(&self.as_str()).finish()
  }
}

impl fmt::Display for BranchName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl From<&str> for BranchName {
  fn from(value: &str) -> Self {
    Self::new(Arc::<str>::from(value))
  }
}

impl From<String> for BranchName {
  fn from(value: String) -> Self {
    Self::new(Arc::<str>::from(value))
  }
}

impl From<Arc<str>> for BranchName {
  fn from(value: Arc<str>) -> Self {
    Self::new(value)
  }
}

/// Directed relationship between two branches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BranchEdge {
  /// Name of the branch the edge originates from (typically the parent).
  pub from: BranchName,
  /// Name of the branch the edge points to (typically the child).
  pub to: BranchName,
}

impl BranchEdge {
  /// Create a new branch edge.
  pub fn new(from: BranchName, to: BranchName) -> Self {
    Self { from, to }
  }
}

/// Categorisation of a branch within the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BranchKind {
  /// A branch that exists locally within the repository.
  Local,
  /// A remote tracking branch (e.g. `origin/main`).
  Remote,
  /// A synthetic node introduced by tooling (for example, dependency hubs that
  /// do not map to concrete Git references).
  Virtual,
}

/// Summary information about the commit referenced by a branch head.
#[derive(Debug, Clone)]
pub struct BranchHead {
  /// Object identifier of the tip commit.
  pub oid: Oid,
  /// Optional human-readable summary (usually the first line of the commit
  /// message).
  pub summary: Option<String>,
  /// Author associated with the commit tip.
  pub author: Option<String>,
  /// Timestamp of the commit in UTC.
  pub committed_at: Option<DateTime<Utc>>,
}

/// Snapshot of metadata useful for renderers or workflows when dealing with a
/// branch.
#[derive(Debug, Clone, Default)]
pub struct BranchNodeMetadata {
  /// Whether the node corresponds to the repository's currently checked-out
  /// branch.
  pub is_current: bool,
  /// Indicates the branch is considered "stale" by analytics.
  pub stale_state: Option<BranchStaleState>,
  /// Arbitrary labels that renderers can surface (e.g. Jira issue keys, PR
  /// numbers).
  pub labels: BTreeSet<String>,
  /// Arbitrary annotations stored as key/value pairs. Callers should namespace
  /// keys (e.g. `jira.ticket`).
  pub annotations: BTreeMap<String, BranchAnnotationValue>,
}

/// Representation of the stale status of a branch.
#[derive(Debug, Clone)]
pub enum BranchStaleState {
  Fresh,
  Stale {
    /// Age in days based on the last commit timestamp.
    age_in_days: u32,
  },
  /// Staleness could not be calculated (e.g. missing commit metadata).
  Unknown,
}

/// Rich value that can be attached to a branch as metadata.
#[derive(Debug, Clone)]
pub enum BranchAnnotationValue {
  Text(String),
  Numeric(i64),
  Timestamp(DateTime<Utc>),
  Flag(bool),
}

/// Node within a branch graph containing topological information and metadata.
#[derive(Debug, Clone)]
pub struct BranchNode {
  /// Canonical name for the branch; duplicates the map key for convenience.
  pub name: BranchName,
  /// Classification for the branch, distinguishing between local, remote, and
  /// synthetic nodes.
  pub kind: BranchKind,
  /// Information about the commit the branch points to.
  pub head: BranchHead,
  /// Upstream branch configured for tracking (if any).
  pub upstream: Option<BranchName>,
  /// Primary and secondary structural relationships.
  pub topology: BranchTopology,
  /// Metadata and annotations attached to the branch.
  pub metadata: BranchNodeMetadata,
}

impl BranchNode {
  /// Convenience accessor to determine whether the node represents the current
  /// branch.
  pub fn is_current(&self) -> bool {
    self.metadata.is_current
  }
}

/// Representation of the relationships between a branch and its neighbours.
#[derive(Debug, Clone, Default)]
pub struct BranchTopology {
  /// Optional primary parent (typically the branch a feature branch was created
  /// from).
  pub primary_parent: Option<BranchName>,
  /// Additional parent branches discovered through heuristics (e.g. merge-base
  /// analysis) or explicit configuration.
  pub secondary_parents: Vec<BranchName>,
  /// Child branches referencing this branch as their primary parent.
  pub children: Vec<BranchName>,
}

/// Graph combining branch nodes and the edges that connect them.
#[derive(Debug, Clone, Default)]
pub struct BranchGraph {
  nodes: BTreeMap<BranchName, BranchNode>,
  edges: Vec<BranchEdge>,
  root_candidates: Vec<BranchName>,
  current_branch: Option<BranchName>,
}

impl BranchGraph {
  /// Create an empty branch graph.
  pub fn new() -> Self {
    Self::default()
  }

  /// Number of nodes recorded in the graph.
  pub fn len(&self) -> usize {
    self.nodes.len()
  }

  /// Returns `true` when the graph does not contain any branches.
  pub fn is_empty(&self) -> bool {
    self.nodes.is_empty()
  }

  /// Iterate over the branch nodes in the graph.
  pub fn iter(&self) -> std::collections::btree_map::Iter<'_, BranchName, BranchNode> {
    self.nodes.iter()
  }

  /// Get a reference to a node by name.
  pub fn get(&self, name: &BranchName) -> Option<&BranchNode> {
    self.nodes.get(name)
  }

  /// Return the branch currently checked out in the repository.
  pub fn current_branch(&self) -> Option<&BranchName> {
    self.current_branch.as_ref()
  }

  /// Candidate roots for rendering the graph. Callers may still choose a
  /// different root depending on UX needs.
  pub fn root_candidates(&self) -> &[BranchName] {
    &self.root_candidates
  }

  /// Access the recorded edges.
  pub fn edges(&self) -> &[BranchEdge] {
    &self.edges
  }
}

/// Errors produced when constructing a branch graph.
#[derive(Debug, Error)]
pub enum BranchGraphError {
  /// The repository is not in a state that allows graph construction (e.g. no
  /// HEAD).
  #[error("repository does not contain a valid HEAD reference")]
  MissingHead,
  /// Placeholder variant signalling that full graph construction has not yet
  /// been implemented.
  #[error("branch graph construction has not been implemented yet")]
  NotImplemented,
  /// Wrapper for lower-level errors originating from `git2`.
  #[error(transparent)]
  Git(#[from] git2::Error),
  /// Wrapper for other error types.
  #[error(transparent)]
  Other(#[from] AnyError),
}

/// Configurable builder responsible for producing a [`BranchGraph`].
///
/// The builder only captures configuration at this stage—the actual graph
/// materialisation will be implemented in a subsequent task. Returning a
/// dedicated error keeps the API stable for future callers.
pub struct BranchGraphBuilder {
  include_remote_branches: bool,
  include_declared_dependencies: bool,
  eager_labels: BTreeSet<String>,
}

impl Default for BranchGraphBuilder {
  fn default() -> Self {
    Self {
      include_remote_branches: false,
      include_declared_dependencies: true,
      eager_labels: BTreeSet::new(),
    }
  }
}

impl BranchGraphBuilder {
  /// Create a new builder with default configuration.
  pub fn new() -> Self {
    Self::default()
  }

  /// Include remote tracking branches in the resulting graph.
  pub fn with_remote_branches(mut self, include: bool) -> Self {
    self.include_remote_branches = include;
    self
  }

  /// Include dependency metadata declared in `.twig/state.json`.
  pub fn with_declared_dependencies(mut self, include: bool) -> Self {
    self.include_declared_dependencies = include;
    self
  }

  /// Request that specific labels are eagerly populated during graph
  /// construction. This gives callers a hook to opt-in to potentially expensive
  /// annotations (e.g. Jira lookups) without requiring extra passes.
  pub fn with_eager_labels<I, S>(mut self, labels: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    self.eager_labels = labels.into_iter().map(Into::into).collect();
    self
  }

  /// Produce a branch graph for the provided repository.
  ///
  /// The implementation will be provided in a follow-up task; for now we
  /// return a stable error variant so downstream callers can begin integrating
  /// against the API.
  pub fn build(self, _repo: &git2::Repository) -> Result<BranchGraph, BranchGraphError> {
    Err(BranchGraphError::NotImplemented)
  }
}
