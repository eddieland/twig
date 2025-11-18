//! # User-Defined Dependency Resolver
//!
//! Resolves explicitly configured branch dependencies to build branch trees
//! based on user-defined relationships rather than Git history analysis.
//!
//! This module provides the `UserDefinedDependencyResolver` struct, which
//! contains methods to resolve user dependencies, build branch trees, and
//! validate dependency integrity. The resolver uses information from the
//! repository state and user-defined configurations to establish parent-child
//! relationships between branches, allowing for a custom view of the branch
//! hierarchy that reflects the user's intentions and configurations.
//!
//! # Usage
//!
//! To use the `UserDefinedDependencyResolver`, create an instance and call
//! its methods with the appropriate parameters. For example, to resolve user
//! dependencies and build branch nodes, use the `resolve_user_dependencies`
//! method. To build a tree structure from user-defined dependencies and roots,
//! use the `build_tree_from_user_dependencies` method.
//!
//! # Example
//!
//! ```
//! let resolver = UserDefinedDependencyResolver;
//! let repo_state = ...; // Obtain repo state
//! let repo = ...; // Obtain Git2 repository instance
//!
//! // Resolve user dependencies and build branch nodes
//! let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;
//!
//! // Build tree structure from user-defined dependencies and roots
//! let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);
//! ```
//!
//! # Notes
//!
//! - The resolver does not modify the actual Git repository or its branches; it
//!   only provides a way to visualize and work with user-defined branch
//!   dependencies.
//! - Dependency cycles are not allowed and will be reported as errors if
//!   detected.
//! - The resolver can suggest a default root branch based on user-defined
//!   settings or common branch naming conventions.

use std::collections::HashMap;

use anyhow::Result;
use git2::{BranchType, Repository as Git2Repository};
use twig_core::RepoState;
use twig_core::tree_renderer::BranchNode;

/// Pure user-defined dependency resolver for tree command
pub struct UserDefinedDependencyResolver;

impl UserDefinedDependencyResolver {
  /// Resolve user-defined dependencies and build branch nodes
  pub fn resolve_user_dependencies(
    &self,
    repo: &Git2Repository,
    repo_state: &RepoState,
  ) -> Result<HashMap<String, BranchNode>> {
    let mut branch_nodes = HashMap::new();

    // Get all local branches
    let branches = repo.branches(Some(BranchType::Local))?;

    // First pass: create all branch nodes
    for branch_result in branches {
      let (branch, _) = branch_result?;
      if let Some(name) = branch.name()? {
        let is_current = branch.is_head();
        let metadata = repo_state.get_branch_metadata(name).cloned();

        let branch_node = BranchNode {
          name: name.to_string(),
          is_current,
          metadata,
          parents: Vec::new(),
          children: Vec::new(),
        };

        branch_nodes.insert(name.to_string(), branch_node);
      }
    }

    // Second pass: build parent-child relationships from user-defined dependencies
    self.build_dependencies_from_state(&mut branch_nodes, repo_state);

    // Attach orphaned branches to the default root (if configured)
    self.attach_orphans_to_default_root(&mut branch_nodes, repo_state);

    Ok(branch_nodes)
  }

  /// Build parent-child relationships from user-defined dependencies
  fn build_dependencies_from_state(&self, branch_nodes: &mut HashMap<String, BranchNode>, repo_state: &RepoState) {
    // Build parent-child relationships from user-defined dependencies
    for dependency in repo_state.list_dependencies() {
      let child_name = &dependency.child;
      let parent_name = &dependency.parent;

      // Only process if both branches exist in our branch nodes
      if branch_nodes.contains_key(child_name) && branch_nodes.contains_key(parent_name) {
        // Add parent to child's parents list
        if let Some(child_node) = branch_nodes.get_mut(child_name)
          && !child_node.parents.contains(parent_name)
        {
          child_node.parents.push(parent_name.clone());
        }

        // Add child to parent's children list
        if let Some(parent_node) = branch_nodes.get_mut(parent_name)
          && !parent_node.children.contains(child_name)
        {
          parent_node.children.push(child_name.clone());
        }
      }
    }
  }

  /// Attach orphaned branches to the default root so they appear under it in
  /// the tree
  fn attach_orphans_to_default_root(&self, branch_nodes: &mut HashMap<String, BranchNode>, repo_state: &RepoState) {
    let Some(default_root) = repo_state.get_default_root() else {
      return;
    };

    if !branch_nodes.contains_key(default_root) {
      return;
    }

    let default_root = default_root.to_string();
    let root_names: std::collections::HashSet<_> = repo_state
      .list_roots()
      .iter()
      .map(|root| root.branch.as_str())
      .collect();

    let orphaned: Vec<_> = branch_nodes
      .iter()
      .filter(|(branch_name, node)| {
        node.parents.is_empty() && !root_names.contains(branch_name.as_str()) && **branch_name != default_root
      })
      .map(|(branch_name, _)| branch_name.clone())
      .collect();

    if orphaned.is_empty() {
      return;
    }

    for branch_name in orphaned {
      if let Some(child_node) = branch_nodes.get_mut(&branch_name)
        && !child_node.parents.contains(&default_root)
      {
        child_node.parents.push(default_root.clone());
      }

      if let Some(parent_node) = branch_nodes.get_mut(&default_root)
        && !parent_node.children.contains(&branch_name)
      {
        parent_node.children.push(branch_name.clone());
      }
    }
  }

  /// Build tree structure from user-defined dependencies and roots
  pub fn build_tree_from_user_dependencies(
    &self,
    branch_nodes: &HashMap<String, BranchNode>,
    repo_state: &RepoState,
  ) -> (Vec<String>, Vec<String>) {
    let mut roots = Vec::new();
    let mut orphaned_branches = Vec::new();

    // Get user-defined root branches
    let user_roots = repo_state.list_roots();

    // If we have user-defined roots, use only those that exist in our branches
    if !user_roots.is_empty() {
      for root in user_roots {
        if branch_nodes.contains_key(&root.branch) {
          roots.push(root.branch.clone());
        }
      }
    }

    // Find branches that have no parents and are not explicitly marked as roots
    for (branch_name, node) in branch_nodes {
      let is_user_root = user_roots.iter().any(|r| r.branch == *branch_name);
      let has_parents = !node.parents.is_empty();

      if !has_parents && !is_user_root {
        // This is an orphaned branch (no dependencies and not a root)
        orphaned_branches.push(branch_name.clone());
      } else if !has_parents && user_roots.is_empty() {
        // If no user-defined roots, treat parentless branches as implicit roots
        roots.push(branch_name.clone());
      }
    }

    // If no roots found and no orphaned branches, treat all branches as orphaned
    if roots.is_empty() && orphaned_branches.is_empty() {
      for branch_name in branch_nodes.keys() {
        orphaned_branches.push(branch_name.clone());
      }
    }

    // Sort both lists for consistent output
    roots.sort();
    orphaned_branches.sort();

    (roots, orphaned_branches)
  }

  /// Validate user dependency integrity (detect cycles, missing branches, etc.)
  #[allow(dead_code)]
  pub fn validate_user_dependency_integrity(&self, _repo_state: &RepoState) -> Result<Vec<String>> {
    let issues = Vec::new();

    // For now, the RepoState already prevents cycles during dependency addition
    // We could add additional validations here in the future:
    // - Check for dependencies referencing non-existent branches
    // - Check for other integrity issues

    Ok(issues)
  }

  /// Get the default root branch or suggest one
  #[allow(dead_code)]
  pub fn get_or_suggest_default_root(
    &self,
    repo_state: &RepoState,
    branch_nodes: &HashMap<String, BranchNode>,
  ) -> Option<String> {
    // First check for user-defined default root
    if let Some(default_root) = repo_state.get_default_root()
      && branch_nodes.contains_key(default_root)
    {
      return Some(default_root.to_string());
    }

    // Look for common root branch names
    let common_roots = ["main", "master", "develop", "dev"];
    for root_name in &common_roots {
      if branch_nodes.contains_key(*root_name) {
        return Some(root_name.to_string());
      }
    }

    // Find current branch as fallback
    for (name, node) in branch_nodes {
      if node.is_current {
        return Some(name.clone());
      }
    }

    // Return first branch alphabetically as last resort
    let mut branch_names: Vec<_> = branch_nodes.keys().cloned().collect();
    branch_names.sort();
    branch_names.into_iter().next()
  }
}

#[cfg(test)]
mod tests {
  use chrono::Utc;
  use twig_core::{BranchDependency, RootBranch};
  use uuid::Uuid;

  use super::*;

  fn create_test_repo_state() -> RepoState {
    let mut state = RepoState::default();

    // Add some test dependencies
    state.dependencies.push(BranchDependency {
      id: Uuid::new_v4(),
      child: "feature/oauth".to_string(),
      parent: "feature/auth".to_string(),
      created_at: Utc::now(),
    });

    state.dependencies.push(BranchDependency {
      id: Uuid::new_v4(),
      child: "feature/2fa".to_string(),
      parent: "feature/auth".to_string(),
      created_at: Utc::now(),
    });

    // Add a root branch
    state.root_branches.push(RootBranch {
      id: Uuid::new_v4(),
      branch: "main".to_string(),
      is_default: true,
      created_at: Utc::now(),
    });

    state
  }

  fn create_test_branch_nodes() -> HashMap<String, BranchNode> {
    let mut nodes = HashMap::new();

    nodes.insert(
      "main".to_string(),
      BranchNode {
        name: "main".to_string(),
        is_current: false,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    nodes.insert(
      "feature/auth".to_string(),
      BranchNode {
        name: "feature/auth".to_string(),
        is_current: false,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    nodes.insert(
      "feature/oauth".to_string(),
      BranchNode {
        name: "feature/oauth".to_string(),
        is_current: true,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    nodes.insert(
      "feature/2fa".to_string(),
      BranchNode {
        name: "feature/2fa".to_string(),
        is_current: false,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    nodes
  }

  #[test]
  fn test_build_dependencies_from_state() {
    let resolver = UserDefinedDependencyResolver;
    let repo_state = create_test_repo_state();
    let mut branch_nodes = create_test_branch_nodes();

    resolver.build_dependencies_from_state(&mut branch_nodes, &repo_state);

    // Check that feature/oauth has feature/auth as parent
    let oauth_node = &branch_nodes["feature/oauth"];
    assert!(oauth_node.parents.contains(&"feature/auth".to_string()));

    // Check that feature/auth has feature/oauth as child
    let auth_node = &branch_nodes["feature/auth"];
    assert!(auth_node.children.contains(&"feature/oauth".to_string()));
    assert!(auth_node.children.contains(&"feature/2fa".to_string()));
  }

  #[test]
  fn test_build_tree_from_user_dependencies() {
    let resolver = UserDefinedDependencyResolver;
    let repo_state = create_test_repo_state();
    let mut branch_nodes = create_test_branch_nodes();

    resolver.build_dependencies_from_state(&mut branch_nodes, &repo_state);
    resolver.attach_orphans_to_default_root(&mut branch_nodes, &repo_state);
    let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

    // Should have main as root
    assert!(roots.contains(&"main".to_string()));

    // Orphaned branches should be attached to the default root
    assert!(branch_nodes["main"].children.contains(&"feature/auth".to_string()));
    assert!(branch_nodes["feature/auth"].parents.contains(&"main".to_string()));
    assert!(!orphaned.contains(&"feature/auth".to_string()));
  }

  #[test]
  fn test_get_or_suggest_default_root() {
    let resolver = UserDefinedDependencyResolver;
    let repo_state = create_test_repo_state();
    let branch_nodes = create_test_branch_nodes();

    let default_root = resolver.get_or_suggest_default_root(&repo_state, &branch_nodes);

    // Should return "main" as it's the user-defined default root
    assert_eq!(default_root, Some("main".to_string()));
  }
}
