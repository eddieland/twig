//! # Auto Dependency Discovery
//!
//! Automatic discovery of Git-based branch dependencies using commit ancestry
//! analysis and heuristics to suggest parent-child relationships between
//! branches.
//!
//! This module preserves the existing treev2 logic for future use

#![allow(dead_code)]

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use git2::{BranchType, Repository as Git2Repository};

use crate::repo_state::RepoState;
use crate::tree_renderer::BranchNode;

pub struct AutoDependencyDiscovery;

#[derive(Debug, Clone)]
pub struct DependencySuggestion {
  pub child: String,
  pub parent: String,
  pub confidence: f64, // 0.0 to 1.0
  pub reason: String,
}

impl AutoDependencyDiscovery {
  /// Discover Git-based dependencies using commit ancestry
  pub fn discover_git_dependencies(
    &self,
    repo: &Git2Repository,
    repo_state: &RepoState,
  ) -> Result<HashMap<String, BranchNode>> {
    let mut branch_nodes = HashMap::new();
    let mut branch_info = HashMap::new();
    let mut root_branches = HashSet::new();

    // Get all local branches
    let branches = repo.branches(Some(BranchType::Local))?;

    // First pass: collect basic branch information
    for branch_result in branches {
      let (branch, _) = branch_result?;
      if let Some(name) = branch.name()? {
        let is_current = branch.is_head();
        let metadata = repo_state.get_branch_issue_by_branch(name).cloned();

        // Get the commit that the branch points to
        let commit = branch.get().peel_to_commit()?;

        // Create a BranchNode for this branch
        let node = BranchNode {
          name: name.to_string(),
          is_current,
          metadata,
          parents: Vec::new(),
          children: Vec::new(),
        };

        branch_nodes.insert(name.to_string(), node);

        // Store commit info for later parent-child relationship resolution
        branch_info.insert(name.to_string(), (commit.id(), is_current));

        // Initially consider all branches as root branches
        root_branches.insert(name.to_string());
      }
    }

    // Second pass: determine parent-child relationships based on Git history
    self.analyze_commit_ancestry(&mut branch_nodes, &branch_info, &mut root_branches, repo)?;

    Ok(branch_nodes)
  }

  /// Analyze commit ancestry to determine branch relationships
  pub fn analyze_commit_ancestry(
    &self,
    branch_nodes: &mut HashMap<String, BranchNode>,
    branch_info: &HashMap<String, (git2::Oid, bool)>,
    root_branches: &mut HashSet<String>,
    repo: &Git2Repository,
  ) -> Result<()> {
    for (branch_name, (commit_id, _)) in branch_info {
      // For each branch, find its parent branches
      let branch_commit = repo.find_commit(*commit_id)?;

      // If the branch has a parent commit, check which branches contain that parent
      if branch_commit.parent_count() > 0 {
        let parent_commit = branch_commit.parent(0)?;

        // Find branches that point to this parent commit or have this commit in their
        // history
        for (other_name, (other_id, _)) in branch_info {
          if other_name == branch_name {
            continue; // Skip self
          }

          // Check if the other branch contains this branch's parent commit
          let mut is_ancestor = false;

          // Use a simpler approach to determine ancestry
          if let Ok(other_commit) = repo.find_commit(*other_id) {
            // Check if we can reach parent_commit from other_commit
            // This is a simplified approach and may not be perfect
            let mut queue = VecDeque::new();
            queue.push_back(other_commit);
            let mut visited = HashSet::new();

            while let Some(current) = queue.pop_front() {
              if current.id() == parent_commit.id() {
                is_ancestor = true;
                break;
              }

              if visited.contains(&current.id()) {
                continue;
              }

              visited.insert(current.id());

              for i in 0..current.parent_count() {
                if let Ok(parent) = current.parent(i) {
                  queue.push_back(parent);
                }
              }
            }
          }

          if is_ancestor {
            // other_branch is a potential parent of branch
            if let Some(node) = branch_nodes.get_mut(branch_name) {
              node.parents.push(other_name.clone());
            }

            // Add branch as a child of other_branch
            if let Some(other_node) = branch_nodes.get_mut(other_name) {
              other_node.children.push(branch_name.clone());
            }

            // This branch is no longer a root branch
            root_branches.remove(branch_name);
          }
        }
      }
    }

    // If no root branches were found, use the current branch or the first branch as
    // root
    if root_branches.is_empty() {
      // Find the current branch or use the first branch
      let root_branch = branch_info
        .iter()
        .find(|(_, (_, is_current))| *is_current)
        .map(|(name, _)| name.clone())
        .or_else(|| branch_info.keys().next().cloned());

      if let Some(name) = root_branch {
        root_branches.insert(name);
      }
    }

    Ok(())
  }

  /// Suggest dependencies based on Git history analysis
  pub fn suggest_dependencies(
    &self,
    repo: &Git2Repository,
    repo_state: &RepoState,
  ) -> Result<Vec<DependencySuggestion>> {
    let branch_nodes = self.discover_git_dependencies(repo, repo_state)?;
    let mut suggestions = Vec::new();

    for (branch_name, node) in &branch_nodes {
      for parent in &node.parents {
        // Skip if this dependency already exists in user-defined dependencies
        let already_exists = repo_state
          .list_dependencies()
          .iter()
          .any(|d| d.child == *branch_name && d.parent == *parent);

        if !already_exists {
          suggestions.push(DependencySuggestion {
            child: branch_name.clone(),
            parent: parent.clone(),
            confidence: 0.8, // Git ancestry has high confidence
            reason: "Based on Git commit ancestry".to_string(),
          });
        }
      }
    }

    Ok(suggestions)
  }

  /// Get root branches from auto-discovery
  pub fn get_auto_discovered_roots(&self, repo: &Git2Repository, _repo_state: &RepoState) -> Result<Vec<String>> {
    let mut branch_info = HashMap::new();
    let mut root_branches = HashSet::new();

    // Get all local branches
    let branches = repo.branches(Some(BranchType::Local))?;

    // Collect branch information
    for branch_result in branches {
      let (branch, _) = branch_result?;
      if let Some(name) = branch.name()? {
        let is_current = branch.is_head();
        let commit = branch.get().peel_to_commit()?;
        branch_info.insert(name.to_string(), (commit.id(), is_current));
        root_branches.insert(name.to_string());
      }
    }

    // Analyze ancestry to remove non-root branches
    let mut branch_nodes = HashMap::new();
    for (name, (_, is_current)) in &branch_info {
      branch_nodes.insert(
        name.clone(),
        BranchNode {
          name: name.clone(),
          is_current: *is_current,
          metadata: None,
          parents: Vec::new(),
          children: Vec::new(),
        },
      );
    }

    self.analyze_commit_ancestry(&mut branch_nodes, &branch_info, &mut root_branches, repo)?;

    let mut roots: Vec<String> = root_branches.into_iter().collect();
    roots.sort();
    Ok(roots)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_dependency_suggestion_creation() {
    let suggestion = DependencySuggestion {
      child: "feature/oauth".to_string(),
      parent: "feature/auth".to_string(),
      confidence: 0.8,
      reason: "Based on Git commit ancestry".to_string(),
    };

    assert_eq!(suggestion.child, "feature/oauth");
    assert_eq!(suggestion.parent, "feature/auth");
    assert_eq!(suggestion.confidence, 0.8);
  }

  #[test]
  fn test_auto_dependency_discovery_creation() {
    let _discovery = AutoDependencyDiscovery;
    // Just test that we can create the struct
    // Actual Git operations would require a real repository
  }
}
