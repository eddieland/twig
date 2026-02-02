//! Branch graph domain models shared between the CLI and `twig` plugins.
//!
//! The goal of this module is to provide a reusable representation of a Git
//! branch tree that can be consumed by renderers (textual trees, tabular
//! summaries, etc.) as well as higher-level workflows like branch switching.
//! The data structures focus on separating the *topology* of a repository from
//! contextual metadata so that additional annotations can be layered on without
//! duplicating graph construction.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use anyhow::Error as AnyError;
use chrono::{DateTime, TimeZone, Utc};
use git2::{self, BranchType, Oid, Repository};
use thiserror::Error;

use crate::state::RepoState;

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
  /// Divergence relative to the branch's primary parent (if any).
  pub divergence: Option<BranchDivergence>,
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

/// Ahead/behind counts for a branch relative to its parent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BranchDivergence {
  /// Commits present on the branch but not on its primary parent.
  pub ahead: usize,
  /// Commits present on the primary parent but missing on the branch.
  pub behind: usize,
}

impl BranchDivergence {
  /// Returns `true` when the branch is fully in sync with its parent.
  pub fn is_zero(&self) -> bool {
    self.ahead == 0 && self.behind == 0
  }
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

  /// Construct a graph from its constituent parts.
  ///
  /// This helper is primarily intended for tests or for callers that already
  /// have branch nodes materialised. The graph builder will eventually
  /// populate these structures directly from a git repository, but exposing
  /// this method keeps the renderer and other consumers decoupled from the
  /// builder implementation progress.
  pub fn from_parts<N, E, R>(nodes: N, edges: E, root_candidates: R, current_branch: Option<BranchName>) -> Self
  where
    N: IntoIterator<Item = BranchNode>,
    E: IntoIterator<Item = BranchEdge>,
    R: IntoIterator<Item = BranchName>,
  {
    let node_map = nodes.into_iter().map(|node| (node.name.clone(), node)).collect();

    Self {
      nodes: node_map,
      edges: edges.into_iter().collect(),
      root_candidates: root_candidates.into_iter().collect(),
      current_branch,
    }
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
  /// The repository does not expose a working directory (bare repository).
  #[error("repository does not have a working directory")]
  MissingWorkdir,
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
  attach_orphans_to_default_root: bool,
}

impl Default for BranchGraphBuilder {
  fn default() -> Self {
    Self {
      include_remote_branches: false,
      include_declared_dependencies: true,
      eager_labels: BTreeSet::new(),
      attach_orphans_to_default_root: false,
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

  /// Treat orphaned branches as children of the default root when rendering.
  pub fn with_orphan_parenting(mut self, attach: bool) -> Self {
    self.attach_orphans_to_default_root = attach;
    self
  }

  /// Produce a branch graph for the provided repository.
  pub fn build(self, repo: &Repository) -> Result<BranchGraph, BranchGraphError> {
    let workdir = repo.workdir().ok_or(BranchGraphError::MissingWorkdir)?;
    let repo_state = RepoState::load(workdir).map_err(BranchGraphError::Other)?;

    let head_branch = repo
      .head()
      .ok()
      .and_then(|head| head.shorthand().map(|s| s.to_string()));
    let mut nodes = self.collect_branches(repo, &repo_state)?;

    if nodes.is_empty() {
      return Ok(BranchGraph::new());
    }

    let mut edges = if self.include_declared_dependencies {
      self.apply_dependencies(&repo_state, &mut nodes)
    } else {
      Vec::new()
    };

    if self.attach_orphans_to_default_root {
      edges.extend(self.attach_orphans_to_default_root(&repo_state, &mut nodes));
    }

    self.apply_divergence(repo, &repo_state, &mut nodes)?;

    let root_candidates = self.resolve_root_candidates(&repo_state, &nodes, head_branch.as_deref());
    let current_branch = head_branch
      .as_ref()
      .and_then(|name| nodes.get(name))
      .map(|node| node.name.clone());

    Ok(BranchGraph::from_parts(
      nodes.into_values(),
      edges,
      root_candidates,
      current_branch,
    ))
  }

  fn collect_branches(
    &self,
    repo: &Repository,
    repo_state: &RepoState,
  ) -> Result<BTreeMap<String, BranchNode>, BranchGraphError> {
    let mut nodes = BTreeMap::new();
    self.collect_branch_type(repo, BranchType::Local, BranchKind::Local, repo_state, &mut nodes)?;

    if self.include_remote_branches {
      self.collect_branch_type(repo, BranchType::Remote, BranchKind::Remote, repo_state, &mut nodes)?;
    }

    Ok(nodes)
  }

  fn collect_branch_type(
    &self,
    repo: &Repository,
    branch_type: BranchType,
    kind: BranchKind,
    repo_state: &RepoState,
    nodes: &mut BTreeMap<String, BranchNode>,
  ) -> Result<(), BranchGraphError> {
    let branches = repo.branches(Some(branch_type))?;

    for branch_result in branches {
      let (branch, _) = branch_result?;
      let Some(name) = branch.name()?.map(|s| s.to_string()) else {
        continue;
      };

      if nodes.contains_key(&name) {
        continue;
      }

      let head = Self::branch_head(&branch)?;
      let upstream = branch
        .upstream()
        .ok()
        .and_then(|upstream| upstream.name().ok().flatten().map(|s| s.to_string()));

      let mut metadata = BranchNodeMetadata {
        is_current: branch.is_head(),
        ..BranchNodeMetadata::default()
      };
      Self::apply_branch_metadata(repo_state, &name, &mut metadata);

      let node = BranchNode {
        name: BranchName::from(name.clone()),
        kind,
        head,
        upstream: upstream.map(BranchName::from),
        topology: BranchTopology::default(),
        metadata,
      };

      nodes.insert(name, node);
    }

    Ok(())
  }

  fn branch_head(branch: &git2::Branch<'_>) -> Result<BranchHead, BranchGraphError> {
    let reference = branch.get();
    let commit = reference.peel_to_commit()?;
    let summary = commit.summary().map(|s| s.to_string());
    let author = commit.author().name().map(|name| name.to_string());
    let time = commit.time();
    let timestamp = time.seconds() + i64::from(time.offset_minutes()) * 60;
    let committed_at = Utc.timestamp_opt(timestamp, 0).single();

    Ok(BranchHead {
      oid: commit.id(),
      summary,
      author,
      committed_at,
    })
  }

  fn apply_branch_metadata(repo_state: &RepoState, branch: &str, metadata: &mut BranchNodeMetadata) {
    if let Some(branch_meta) = repo_state.get_branch_metadata(branch) {
      if let Some(jira) = &branch_meta.jira_issue {
        metadata.labels.insert(jira.clone());
      }

      if let Some(pr) = branch_meta.github_pr {
        metadata
          .annotations
          .insert("twig.pr".to_string(), BranchAnnotationValue::Numeric(pr as i64));
      }
    }
  }

  fn apply_dependencies(&self, repo_state: &RepoState, nodes: &mut BTreeMap<String, BranchNode>) -> Vec<BranchEdge> {
    let mut edges = Vec::new();
    let mut parents_by_child: BTreeMap<String, Vec<BranchName>> = BTreeMap::new();

    for dependency in repo_state.list_dependencies() {
      if nodes.contains_key(&dependency.child) && nodes.contains_key(&dependency.parent) {
        let parent_name = nodes
          .get(&dependency.parent)
          .map(|node| node.name.clone())
          .expect("checked via contains_key");
        parents_by_child
          .entry(dependency.child.clone())
          .or_default()
          .push(parent_name);
      }
    }

    for dependency in repo_state.list_dependencies() {
      if let Some(child_branch) = nodes.get(&dependency.child).map(|node| node.name.clone())
        && let Some(parent_node) = nodes.get_mut(&dependency.parent)
      {
        if !parent_node.topology.children.iter().any(|child| child == &child_branch) {
          parent_node.topology.children.push(child_branch.clone());
        }
        edges.push(BranchEdge::new(parent_node.name.clone(), child_branch));
      }
    }

    for node in nodes.values_mut() {
      node.topology.children.sort();
    }

    for (child, parents) in parents_by_child {
      if let Some(child_node) = nodes.get_mut(&child) {
        if let Some(primary) = parents.first() {
          child_node.topology.primary_parent = Some(primary.clone());
        }
        child_node.topology.secondary_parents = parents.iter().skip(1).cloned().collect();
      }
    }

    edges
  }

  fn attach_orphans_to_default_root(
    &self,
    repo_state: &RepoState,
    nodes: &mut BTreeMap<String, BranchNode>,
  ) -> Vec<BranchEdge> {
    let default_root = repo_state
      .get_default_root()
      .map(str::to_string)
      .or_else(|| repo_state.get_root_branches().first().cloned());

    let Some(default_root) = default_root else {
      return Vec::new();
    };

    let Some(root_node_name) = nodes.get(&default_root).map(|node| node.name.clone()) else {
      return Vec::new();
    };

    let configured_roots: BTreeSet<String> = repo_state.get_root_branches().into_iter().collect();
    let orphan_names: Vec<String> = nodes
      .iter()
      .filter(|(name, node)| {
        node.topology.primary_parent.is_none() && *name != &default_root && !configured_roots.contains(*name)
      })
      .map(|(name, _)| name.clone())
      .collect();

    if orphan_names.is_empty() {
      return Vec::new();
    }

    let mut child_names = Vec::new();
    for orphan_name in &orphan_names {
      if let Some(orphan_node) = nodes.get_mut(orphan_name) {
        orphan_node.topology.primary_parent = Some(root_node_name.clone());
        child_names.push(orphan_node.name.clone());
      }
    }

    let mut edges = Vec::new();
    if let Some(root_node) = nodes.get_mut(&default_root) {
      let existing_children: HashSet<_> = root_node.topology.children.iter().cloned().collect();
      let new_children: Vec<_> = child_names
        .iter()
        .filter(|name| !existing_children.contains(*name))
        .cloned()
        .collect();
      root_node.topology.children.extend(new_children);
      for child_name in &child_names {
        edges.push(BranchEdge::new(root_node_name.clone(), child_name.clone()));
      }
      root_node.topology.children.sort();
    }

    edges
  }

  fn apply_divergence(
    &self,
    repo: &Repository,
    repo_state: &RepoState,
    nodes: &mut BTreeMap<String, BranchNode>,
  ) -> Result<(), BranchGraphError> {
    let mut cached_counts: HashMap<(Oid, Oid), (usize, usize)> = HashMap::new();
    let head_by_name: BTreeMap<String, Oid> = nodes.iter().map(|(name, node)| (name.clone(), node.head.oid)).collect();

    // Get default root for orphan comparison
    let default_root = repo_state
      .get_default_root()
      .map(str::to_string)
      .or_else(|| repo_state.get_root_branches().first().cloned());

    for node in nodes.values_mut() {
      // Determine comparison target: primary_parent OR default_root for orphans
      let comparison_branch = node.topology.primary_parent.clone().map(|p| p.to_string()).or_else(|| {
        // Orphan: use default root if available and not self
        default_root
          .as_ref()
          .filter(|root| *root != node.name.as_str())
          .cloned()
      });

      let Some(parent_name) = comparison_branch else {
        continue;
      };

      let Some(parent_head) = head_by_name.get(&parent_name) else {
        continue;
      };

      let child_head = node.head.oid;
      let key = (child_head, *parent_head);
      let (ahead, behind) = match cached_counts.get(&key) {
        Some(counts) => *counts,
        None => {
          let counts = repo.graph_ahead_behind(child_head, *parent_head)?;
          cached_counts.insert(key, counts);
          counts
        }
      };

      node.metadata.divergence = Some(BranchDivergence { ahead, behind });
    }

    Ok(())
  }

  fn resolve_root_candidates(
    &self,
    repo_state: &RepoState,
    nodes: &BTreeMap<String, BranchNode>,
    head_branch: Option<&str>,
  ) -> Vec<BranchName> {
    let mut roots = Vec::new();

    for root in repo_state.get_root_branches() {
      if let Some(node) = nodes.get(&root) {
        roots.push(node.name.clone());
      }
    }

    if roots.is_empty()
      && let Some(head) = head_branch
      && let Some(node) = nodes.get(head)
    {
      roots.push(node.name.clone());
    }

    if roots.is_empty()
      && let Some(node) = nodes.values().next()
    {
      roots.push(node.name.clone());
    }

    roots
  }
}

#[cfg(test)]
mod tests {
  use chrono::Utc;
  use twig_test_utils::git::{GitRepoTestGuard, checkout_branch, create_branch, create_commit};

  use super::*;
  use crate::state::{BranchMetadata, RepoState};

  #[test]
  fn builds_graph_with_dependencies_and_metadata() {
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "README.md", "hello", "initial").unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let head_name = repo.head().unwrap().shorthand().unwrap().to_string();
    repo.branch("feature/payment", &head, false).unwrap();

    let workdir = repo.workdir().unwrap();

    let mut state = RepoState::default();
    state.add_root(head_name.clone(), true).unwrap();
    state.add_branch_issue(BranchMetadata {
      branch: "feature/payment".into(),
      jira_issue: Some("PROJ-123".into()),
      github_pr: Some(42),
      created_at: Utc::now().to_rfc3339(),
    });
    state
      .add_dependency("feature/payment".to_string(), head_name.clone())
      .unwrap();
    state.save(workdir).unwrap();

    let graph = BranchGraphBuilder::new().build(repo).unwrap();
    assert_eq!(graph.len(), 2);

    let root = graph.root_candidates().first().unwrap();
    assert_eq!(root.as_str(), head_name);

    let feature = graph.get(&BranchName::from("feature/payment")).unwrap();
    assert_eq!(feature.topology.primary_parent.as_ref().unwrap().as_str(), head_name);
    assert!(feature.metadata.labels.contains("PROJ-123"));
    match feature.metadata.annotations.get("twig.pr").unwrap() {
      BranchAnnotationValue::Numeric(value) => assert_eq!(*value, 42),
      other => panic!("unexpected annotation value: {other:?}"),
    }
  }

  #[test]
  fn records_divergence_against_primary_parent() {
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "README.md", "hello", "initial").unwrap();
    let main_branch = repo.head().unwrap().shorthand().unwrap().to_string();
    create_branch(repo, "feature/delta", None).unwrap();

    checkout_branch(repo, "feature/delta").unwrap();
    create_commit(repo, "feature.txt", "content", "feature work").unwrap();

    checkout_branch(repo, &main_branch).unwrap();
    create_commit(repo, "main.txt", "more", "main work").unwrap();

    let workdir = repo.workdir().unwrap();

    let mut state = RepoState::default();
    state
      .add_dependency("feature/delta".into(), main_branch.clone())
      .unwrap();
    state.save(workdir).unwrap();

    let graph = BranchGraphBuilder::new().build(repo).unwrap();
    let branch = graph.get(&BranchName::from("feature/delta")).unwrap();
    let divergence = branch.metadata.divergence.as_ref().expect("divergence recorded");

    assert_eq!(divergence.ahead, 1);
    assert_eq!(divergence.behind, 1);
  }

  #[test]
  fn attaches_orphans_to_default_root_when_enabled() {
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "README.md", "hello", "initial").unwrap();
    let main_branch = repo.head().unwrap().shorthand().unwrap().to_string();
    create_branch(repo, "feature/stray", None).unwrap();

    let workdir = repo.workdir().unwrap();
    let mut state = RepoState::default();
    state.add_root(main_branch.clone(), true).unwrap();
    state.save(workdir).unwrap();

    let graph = BranchGraphBuilder::new()
      .with_orphan_parenting(true)
      .build(repo)
      .unwrap();

    let root = graph.get(&BranchName::from(main_branch.as_str())).unwrap();
    assert!(
      root
        .topology
        .children
        .iter()
        .any(|child| child.as_str() == "feature/stray")
    );

    let orphan = graph.get(&BranchName::from("feature/stray")).unwrap();
    assert_eq!(
      orphan.topology.primary_parent.as_ref().map(BranchName::as_str),
      Some(main_branch.as_str())
    );

    assert!(
      graph
        .edges()
        .iter()
        .any(|edge| edge.from.as_str() == main_branch && edge.to.as_str() == "feature/stray")
    );
  }

  #[test]
  fn records_divergence_for_orphans_against_default_root() {
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "README.md", "hello", "initial").unwrap();
    let main_branch = repo.head().unwrap().shorthand().unwrap().to_string();
    create_branch(repo, "feature/orphan", None).unwrap();

    checkout_branch(repo, "feature/orphan").unwrap();
    create_commit(repo, "orphan.txt", "content", "orphan work 1").unwrap();
    create_commit(repo, "orphan2.txt", "more", "orphan work 2").unwrap();

    checkout_branch(repo, &main_branch).unwrap();
    create_commit(repo, "main.txt", "more", "main work").unwrap();

    let workdir = repo.workdir().unwrap();

    // Configure main as root but NO dependency for the orphan branch
    let mut state = RepoState::default();
    state.add_root(main_branch.clone(), true).unwrap();
    state.save(workdir).unwrap();

    let graph = BranchGraphBuilder::new().build(repo).unwrap();
    let orphan = graph.get(&BranchName::from("feature/orphan")).unwrap();

    // Orphan should have no primary_parent since we didn't configure a dependency
    assert!(orphan.topology.primary_parent.is_none());

    // But divergence should still be calculated against the default root
    let divergence = orphan
      .metadata
      .divergence
      .as_ref()
      .expect("divergence recorded for orphan");
    assert_eq!(divergence.ahead, 2); // Two commits on orphan
    assert_eq!(divergence.behind, 1); // One commit on main after branch point
  }

  #[test]
  fn no_divergence_for_default_root_itself() {
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "README.md", "hello", "initial").unwrap();
    let main_branch = repo.head().unwrap().shorthand().unwrap().to_string();

    let workdir = repo.workdir().unwrap();
    let mut state = RepoState::default();
    state.add_root(main_branch.clone(), true).unwrap();
    state.save(workdir).unwrap();

    let graph = BranchGraphBuilder::new().build(repo).unwrap();
    let root = graph.get(&BranchName::from(main_branch.as_str())).unwrap();

    // Default root should not have divergence (no self-comparison)
    assert!(root.metadata.divergence.is_none());
  }
}
