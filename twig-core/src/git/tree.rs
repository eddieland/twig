//! Tree traversal and filtering utilities for branch graphs.
//!
//! This module provides functions for determining render roots, finding
//! orphaned branches, and filtering branch graphs by patterns.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::git::{BranchAnnotationValue, BranchEdge, BranchGraph, BranchName, ORPHAN_BRANCH_ANNOTATION_KEY};
use crate::state::RepoState;

/// Determines the best root branch to use for rendering the tree.
///
/// The function uses the following priority order:
/// 1. An explicit override branch if provided and exists in the graph
/// 2. The configured default root branch if it exists in the graph
/// 3. The first root candidate from the graph
/// 4. The current branch
/// 5. Any branch in the graph as a fallback
pub fn determine_render_root(
  graph: &BranchGraph,
  state: &RepoState,
  override_branch: Option<String>,
) -> Option<BranchName> {
  if let Some(branch) = override_branch {
    let target = BranchName::from(branch.clone());
    if graph.get(&target).is_some() {
      return Some(target);
    }
  }

  if let Some(root) = state.get_default_root() {
    let candidate = BranchName::from(root.to_string());
    if graph.get(&candidate).is_some() {
      return Some(candidate);
    }
  }

  if let Some(candidate) = graph.root_candidates().first() {
    return Some(candidate.clone());
  }

  if let Some(branch) = graph.current_branch() {
    return Some(branch.clone());
  }

  graph.iter().next().map(|(_, node)| node.name.clone())
}

/// Finds branches that have no configured dependencies and are not root
/// branches.
///
/// An orphaned branch is one that:
/// - Has no parent dependencies configured in the repo state
/// - Is not marked as a root branch
pub fn find_orphaned_branches(graph: &BranchGraph, repo_state: &RepoState) -> BTreeSet<BranchName> {
  let configured_roots: HashSet<_> = repo_state.get_root_branches().into_iter().collect();

  graph
    .iter()
    .filter_map(|(name, _)| {
      let branch = name.as_str();
      let has_parent = !repo_state.get_dependency_parents(branch).is_empty();
      if has_parent || configured_roots.contains(branch) {
        None
      } else {
        Some(name.clone())
      }
    })
    .collect()
}

/// Returns the default root branch from state, or the first configured root.
pub fn default_root_branch(state: &RepoState) -> Option<String> {
  state
    .get_default_root()
    .map(|root| root.to_string())
    .or_else(|| state.get_root_branches().first().cloned())
}

/// Attaches orphaned branches to the default root branch in the graph.
///
/// This modifies the graph topology so that branches without a configured
/// parent are shown as children of the default root branch, making the tree
/// visualization more complete.
pub fn attach_orphans_to_default_root(graph: BranchGraph, repo_state: &RepoState) -> BranchGraph {
  let Some(default_root) = default_root_branch(repo_state) else {
    return graph;
  };

  let default_root_name = BranchName::from(default_root.as_str());

  // Check if root exists in graph before extracting parts
  let Some(root_node) = graph.get(&default_root_name) else {
    return graph;
  };
  let root_node_name = root_node.name.clone();

  // Collect configured roots and orphan names before extracting parts
  let configured_roots: HashSet<_> = repo_state.get_root_branches().into_iter().collect();
  let orphan_names: Vec<BranchName> = graph
    .iter()
    .filter_map(|(name, node)| {
      if node.topology.primary_parent.is_none()
        && name != &default_root_name
        && !configured_roots.contains(name.as_str())
      {
        Some(name.clone())
      } else {
        None
      }
    })
    .collect();

  if orphan_names.is_empty() {
    return graph;
  }

  // Only extract parts after confirming we have work to do
  let (mut nodes, mut edges, root_candidates, current_branch) = graph.into_parts();

  let mut child_names = Vec::new();
  for orphan_name in &orphan_names {
    if let Some(orphan_node) = nodes.get_mut(orphan_name) {
      orphan_node.topology.primary_parent = Some(root_node_name.clone());
      child_names.push(orphan_node.name.clone());
    }
  }

  if let Some(root_node) = nodes.get_mut(&root_node_name) {
    // Pre-collect existing children for O(1) membership checks
    let existing_children: HashSet<_> = root_node.topology.children.iter().cloned().collect();
    for child_name in &child_names {
      if !existing_children.contains(child_name) {
        root_node.topology.children.push(child_name.clone());
      }
      edges.push(BranchEdge::new(root_node_name.clone(), child_name.clone()));
    }
    root_node.topology.children.sort();
  }

  BranchGraph::from_parts(nodes.into_values(), edges, root_candidates, current_branch)
}

/// Annotates branches as orphaned by adding an annotation flag.
///
/// This allows the renderer to display a visual indicator for branches that
/// don't have configured dependencies.
pub fn annotate_orphaned_branches(graph: BranchGraph, orphaned: &BTreeSet<BranchName>) -> BranchGraph {
  if orphaned.is_empty() {
    return graph;
  }

  let (mut nodes, edges, root_candidates, current_branch) = graph.into_parts();

  // Only modify nodes that are orphaned, avoiding clones of non-orphaned nodes
  for name in orphaned {
    if let Some(node) = nodes.get_mut(name) {
      node.metadata.annotations.insert(
        ORPHAN_BRANCH_ANNOTATION_KEY.to_string(),
        BranchAnnotationValue::Flag(true),
      );
    }
  }

  BranchGraph::from_parts(nodes.into_values(), edges, root_candidates, current_branch)
}

/// Filters a branch graph to include only branches matching a pattern and their
/// ancestors.
///
/// Returns the filtered graph and the set of branches that directly matched the
/// pattern. The pattern is matched case-insensitively against branch names.
///
/// Returns `None` if no branches match the pattern.
pub fn filter_branch_graph(graph: &BranchGraph, pattern: &str) -> Option<(BranchGraph, BTreeSet<BranchName>)> {
  let needle = pattern.to_lowercase();
  let mut matches = BTreeSet::new();

  for (name, _) in graph.iter() {
    if name.as_str().to_lowercase().contains(&needle) {
      matches.insert(name.clone());
    }
  }

  if matches.is_empty() {
    return None;
  }

  let mut allowed = matches.clone();
  let mut stack: Vec<BranchName> = matches.iter().cloned().collect();

  while let Some(current) = stack.pop() {
    if let Some(node) = graph.get(&current)
      && let Some(parent) = node.topology.primary_parent.as_ref()
      && allowed.insert(parent.clone())
    {
      stack.push(parent.clone());
    }
  }

  let mut nodes = BTreeMap::new();
  for (name, node) in graph.iter() {
    if allowed.contains(name) {
      let mut filtered_node = node.clone();
      filtered_node.topology.children.retain(|child| allowed.contains(child));
      nodes.insert(name.clone(), filtered_node);
    }
  }

  let edges = graph
    .edges()
    .iter()
    .filter(|edge| allowed.contains(&edge.from) && allowed.contains(&edge.to))
    .cloned()
    .collect::<Vec<BranchEdge>>();

  let root_candidates = graph
    .root_candidates()
    .iter()
    .filter(|candidate| allowed.contains(candidate))
    .cloned()
    .collect::<Vec<BranchName>>();

  let current_branch = graph.current_branch().filter(|name| allowed.contains(name)).cloned();

  Some((
    BranchGraph::from_parts(nodes.into_values(), edges, root_candidates, current_branch),
    matches,
  ))
}

#[cfg(test)]
mod tests {
  use git2::Oid;

  use super::*;
  use crate::git::{BranchHead, BranchKind, BranchNode, BranchTopology};

  fn branch_node(name: &str) -> BranchNode {
    BranchNode {
      name: BranchName::from(name),
      kind: BranchKind::Local,
      head: BranchHead {
        oid: Oid::from_str("0123456789abcdef0123456789abcdef01234567").expect("valid oid"),
        summary: Some(format!("Summary for {name}")),
        author: Some("Twig Bot".to_string()),
        committed_at: None,
      },
      upstream: None,
      topology: BranchTopology::default(),
      metadata: Default::default(),
    }
  }

  #[test]
  fn filter_keeps_matching_branches_and_ancestors() {
    let mut root = branch_node("main");
    let mut feature = branch_node("feature/payment");
    feature.topology.primary_parent = Some(root.name.clone());
    root.topology.children.push(feature.name.clone());

    let mut api = branch_node("feature/payment-api");
    api.topology.primary_parent = Some(feature.name.clone());
    feature.topology.children.push(api.name.clone());

    let mut ui = branch_node("feature/payment-ui");
    ui.topology.primary_parent = Some(feature.name.clone());
    feature.topology.children.push(ui.name.clone());

    let mut other = branch_node("feature/other");
    other.topology.primary_parent = Some(root.name.clone());
    root.topology.children.push(other.name.clone());

    let edges = vec![
      BranchEdge::new(root.name.clone(), feature.name.clone()),
      BranchEdge::new(feature.name.clone(), api.name.clone()),
      BranchEdge::new(feature.name.clone(), ui.name.clone()),
      BranchEdge::new(root.name.clone(), other.name.clone()),
    ];

    let graph = BranchGraph::from_parts(
      vec![root.clone(), feature.clone(), api.clone(), ui.clone(), other.clone()],
      edges,
      vec![root.name.clone()],
      Some(root.name.clone()),
    );

    let (filtered, matches) = filter_branch_graph(&graph, "api").expect("expected matches");

    assert!(matches.contains(&api.name));
    assert_eq!(matches.len(), 1);
    assert!(filtered.get(&api.name).is_some());
    assert!(filtered.get(&feature.name).is_some());
    assert!(filtered.get(&root.name).is_some());
    assert!(filtered.get(&ui.name).is_none());
    assert!(filtered.get(&other.name).is_none());

    let parent = filtered
      .get(&api.name)
      .and_then(|node| node.topology.primary_parent.as_ref())
      .expect("parent retained");
    assert_eq!(parent, &feature.name);
  }

  #[test]
  fn find_orphaned_branches_returns_branches_without_parents() {
    let root = branch_node("main");
    let orphan = branch_node("orphan-branch");
    let child = branch_node("child-branch");

    let graph = BranchGraph::from_parts(
      vec![root.clone(), orphan.clone(), child.clone()],
      vec![],
      vec![root.name.clone()],
      None,
    );

    let mut state = RepoState::default();
    state.add_root("main".to_string(), true).expect("add root");
    state
      .add_dependency("child-branch".to_string(), "main".to_string())
      .expect("add dep");

    let orphaned = find_orphaned_branches(&graph, &state);

    assert!(orphaned.contains(&BranchName::from("orphan-branch")));
    assert!(!orphaned.contains(&BranchName::from("main")));
    assert!(!orphaned.contains(&BranchName::from("child-branch")));
  }

  #[test]
  fn determine_render_root_uses_override_first() {
    let root = branch_node("main");
    let feature = branch_node("feature");

    let graph = BranchGraph::from_parts(
      vec![root.clone(), feature.clone()],
      vec![],
      vec![root.name.clone()],
      None,
    );

    let state = RepoState::default();

    let result = determine_render_root(&graph, &state, Some("feature".to_string()));
    assert_eq!(result, Some(BranchName::from("feature")));
  }

  #[test]
  fn determine_render_root_falls_back_to_default_root() {
    let root = branch_node("main");
    let feature = branch_node("feature");

    let graph = BranchGraph::from_parts(
      vec![root.clone(), feature.clone()],
      vec![],
      vec![root.name.clone()],
      None,
    );

    let mut state = RepoState::default();
    state.add_root("main".to_string(), true).expect("add root");

    let result = determine_render_root(&graph, &state, None);
    assert_eq!(result, Some(BranchName::from("main")));
  }
}
