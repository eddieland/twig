//! Data structures and configuration types for representing Git branch graphs.
//!
//! The `twig flow` plugin needs to reason about branch ancestry in a way that
//! is reusable for other commands.  This module defines the domain model and
//! the configuration surface that future builders will populate from Git
//! repositories or cached Twig metadata.  The goal is to keep the structures
//! independent from IO so they can be unit tested and reused across binaries
//! and plugins.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use git2::Oid;
use thiserror::Error;

/// A cheap-to-clone wrapper around a branch name.
///
/// Branch graph operations frequently pass branch identifiers by value when
/// constructing ancestry relationships.  Wrapping the string in an `Arc` avoids
/// repeated allocations without forcing callers to depend on any particular
/// interning implementation.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BranchName(Arc<str>);

impl BranchName {
  /// Creates a new branch name owned by the graph.
  pub fn new(name: impl Into<Arc<str>>) -> Self {
    Self(name.into())
  }

  /// Returns the underlying branch name as a string slice.
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Debug for BranchName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl From<&str> for BranchName {
  fn from(value: &str) -> Self {
    Self::new(Arc::from(value))
  }
}

impl From<String> for BranchName {
  fn from(value: String) -> Self {
    Self::new(Arc::from(value))
  }
}

impl Borrow<str> for BranchName {
  fn borrow(&self) -> &str {
    self.as_str()
  }
}

impl AsRef<str> for BranchName {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

/// Classification of a branch node within the graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchKind {
  /// A local branch stored under `refs/heads/*`.
  Local,
  /// A remote-tracking branch such as `origin/main`.
  Remote,
  /// A synthetic or cached branch that does not exist in the Git repository
  /// but is stored in Twig state (e.g., stale branch analytics).
  Virtual,
}

/// Metadata describing a branch beyond its name and head commit.
#[derive(Clone, Debug, Default)]
pub struct BranchMetadata {
  /// Indicates that the branch matches the repository's current `HEAD`.
  pub is_current: bool,
  /// The optional upstream branch configured for the branch.
  pub upstream: Option<BranchName>,
  /// Issue or ticket keys associated with the branch.
  pub issue_keys: Vec<String>,
  /// Additional free-form labels derived from Twig registry metadata.
  pub labels: BTreeSet<String>,
  /// Timestamp of the branch's most recent commit if it is known.
  pub last_commit_time: Option<DateTime<Utc>>,
}

/// Captures the parent/child relationships for a branch.
#[derive(Clone, Debug, Default)]
pub struct BranchAncestry {
  parent: Option<BranchName>,
  children: BTreeSet<BranchName>,
}

impl BranchAncestry {
  /// Creates a new ancestry record without any links.
  pub fn new() -> Self {
    Self::default()
  }

  /// Records the parent branch that the current node descends from.
  pub fn set_parent(&mut self, parent: Option<BranchName>) {
    self.parent = parent;
  }

  /// Adds a child branch to the ancestry set.
  pub fn add_child(&mut self, child: BranchName) {
    self.children.insert(child);
  }

  /// Returns the configured parent branch, if any.
  pub fn parent(&self) -> Option<&BranchName> {
    self.parent.as_ref()
  }

  /// Returns an iterator over the node's children ordered lexicographically
  /// by branch name.
  pub fn children(&self) -> impl Iterator<Item = &BranchName> {
    self.children.iter()
  }
}

/// A node representing a single branch within the graph.
#[derive(Clone, Debug)]
pub struct BranchNode {
  name: BranchName,
  kind: BranchKind,
  head: Oid,
  metadata: BranchMetadata,
  ancestry: BranchAncestry,
}

impl BranchNode {
  /// Creates a new branch node with the provided metadata and ancestry.
  pub fn new(
    name: BranchName,
    kind: BranchKind,
    head: Oid,
    metadata: BranchMetadata,
    ancestry: BranchAncestry,
  ) -> Self {
    Self {
      name,
      kind,
      head,
      metadata,
      ancestry,
    }
  }

  /// Returns the branch's name.
  pub fn name(&self) -> &BranchName {
    &self.name
  }

  /// Returns the branch classification.
  pub fn kind(&self) -> BranchKind {
    self.kind
  }

  /// Returns the commit hash that the branch currently points to.
  pub fn head(&self) -> Oid {
    self.head
  }

  /// Returns additional metadata associated with the branch.
  pub fn metadata(&self) -> &BranchMetadata {
    &self.metadata
  }

  /// Returns the ancestry relationships for the branch.
  pub fn ancestry(&self) -> &BranchAncestry {
    &self.ancestry
  }
}

/// Represents a set of branches and the relationships between them.
#[derive(Clone, Debug, Default)]
pub struct BranchGraph {
  nodes: BTreeMap<BranchName, BranchNode>,
  /// Optional default root to start rendering or traversal from.
  default_root: Option<BranchName>,
}

impl BranchGraph {
  /// Creates an empty branch graph.
  pub fn new() -> Self {
    Self::default()
  }

  /// Inserts a node into the graph, returning the previous node if present.
  pub fn insert(&mut self, node: BranchNode) -> Option<BranchNode> {
    if self.default_root.is_none() && node.metadata.is_current {
      self.default_root = Some(node.name.clone());
    }
    self.nodes.insert(node.name.clone(), node)
  }

  /// Returns an iterator over the nodes ordered lexicographically by branch
  /// name.  This ordering keeps rendering deterministic.
  pub fn iter(&self) -> impl Iterator<Item = &BranchNode> {
    self.nodes.values()
  }

  /// Fetches a node by branch name.
  pub fn get(&self, name: &str) -> Option<&BranchNode> {
    self.nodes.get(name)
  }

  /// Returns the default root branch if one was recorded.
  pub fn default_root(&self) -> Option<&BranchName> {
    self.default_root.as_ref()
  }

  /// Returns the number of branches stored in the graph.
  pub fn len(&self) -> usize {
    self.nodes.len()
  }

  /// Indicates whether the graph contains any branches.
  pub fn is_empty(&self) -> bool {
    self.nodes.is_empty()
  }
}

/// Selection strategy for focusing the graph during rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchGraphFocus {
  /// Use the graph's default root (typically the current branch).
  Default,
  /// Focus on a specific root branch without mutating the repository.
  Root { branch: BranchName },
  /// Checkout the parent branch before rendering its subtree.
  Parent { branch: BranchName },
}

impl Default for BranchGraphFocus {
  fn default() -> Self {
    Self::Default
  }
}

/// Options that control how a `BranchGraph` is built or rendered.
#[derive(Clone, Debug, Default)]
pub struct BranchGraphOptions {
  /// Strategy for choosing the branch that anchors the rendered tree.
  pub focus: BranchGraphFocus,
  /// Maximum depth to traverse when populating descendants.  `None` means no
  /// limit.
  pub max_depth: Option<usize>,
  /// Whether remote-tracking branches should be included alongside local
  /// branches.
  pub include_remotes: bool,
}

/// Errors that may occur while constructing a branch graph.
#[derive(Debug, Error)]
pub enum BranchGraphError {
  /// The repository does not contain the requested branch.
  #[error("branch '{0}' not found")]
  BranchNotFound(String),
  /// The repository lacks a Git work tree or HEAD reference.
  #[error("unable to determine repository HEAD: {0}")]
  DetachedHead(String),
  /// An invalid combination of options was supplied.
  #[error("incompatible branch graph options: {0}")]
  InvalidOptions(String),
  /// Wrapper around underlying `git2` errors.
  #[error(transparent)]
  Git(#[from] git2::Error),
}

/// Shared trait for components that can build `BranchGraph` instances.
pub trait BranchGraphProvider {
  /// Constructs a `BranchGraph` using the supplied options.
  fn build_graph(&self, options: &BranchGraphOptions) -> Result<BranchGraph, BranchGraphError>;
}
