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
  use std::path::Path;

  use git2::BranchType;
  use twig_test_utils::git::{GitRepoTestGuard, checkout_branch, create_branch, create_commit};

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
  fn test_discover_git_dependencies_simple_chain() -> Result<()> {
    // Create a temporary git repository
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    // Create main branch explicitly
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Create sub-feature branch from feature
    create_branch(repo, "sub-feature", Some("feature"))?;
    checkout_branch(repo, "sub-feature")?;
    create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

    // Initialize repo state
    let repo_state = RepoState::load(repo_path).unwrap_or_default();

    // Run auto dependency discovery
    let discovery = AutoDependencyDiscovery;
    let branch_nodes = discovery.discover_git_dependencies(repo, &repo_state)?;

    // Verify the discovered dependencies
    assert!(branch_nodes.contains_key("main"));
    assert!(branch_nodes.contains_key("feature"));
    assert!(branch_nodes.contains_key("sub-feature"));

    // Check parent-child relationships
    let main_node = &branch_nodes["main"];
    let feature_node = &branch_nodes["feature"];
    let sub_feature_node = &branch_nodes["sub-feature"];

    // Main should have feature as a child
    assert!(main_node.children.contains(&"feature".to_string()));

    // Feature should have main as a parent and sub-feature as a child
    assert!(feature_node.parents.contains(&"main".to_string()));
    assert!(feature_node.children.contains(&"sub-feature".to_string()));

    // Sub-feature should have feature as a parent
    assert!(sub_feature_node.parents.contains(&"feature".to_string()));

    Ok(())
  }

  #[test]
  fn test_suggest_dependencies() -> Result<()> {
    // Create a temporary git repository
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    // Create main branch explicitly
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Create sub-feature branch from feature
    create_branch(repo, "sub-feature", Some("feature"))?;
    checkout_branch(repo, "sub-feature")?;
    create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

    // Initialize repo state
    let repo_state = RepoState::load(repo_path).unwrap_or_default();

    // Run auto dependency discovery to suggest dependencies
    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Verify the suggested dependencies
    assert!(!suggestions.is_empty());

    // Check for specific suggestions
    let has_feature_main = suggestions
      .iter()
      .any(|s| s.child == "feature" && s.parent == "main" && s.confidence > 0.0);

    let has_subfeature_feature = suggestions
      .iter()
      .any(|s| s.child == "sub-feature" && s.parent == "feature" && s.confidence > 0.0);

    assert!(has_feature_main, "Should suggest feature depends on main");
    assert!(has_subfeature_feature, "Should suggest sub-feature depends on feature");

    Ok(())
  }

  #[test]
  fn test_suggest_dependencies_with_existing_dependencies() -> Result<()> {
    // Create a temporary git repository
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    // Create main branch explicitly
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Create sub-feature branch from feature
    create_branch(repo, "sub-feature", Some("feature"))?;
    checkout_branch(repo, "sub-feature")?;
    create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

    // Initialize repo state and add a user-defined dependency
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
    repo_state.add_dependency("feature".to_string(), "main".to_string())?;
    repo_state.save(repo_path)?;

    // Reload the repo state
    let repo_state = RepoState::load(repo_path)?;

    // Run auto dependency discovery to suggest dependencies
    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Verify the suggested dependencies
    // The feature->main dependency should not be suggested since it already exists
    let has_feature_main = suggestions.iter().any(|s| s.child == "feature" && s.parent == "main");

    let has_subfeature_feature = suggestions
      .iter()
      .any(|s| s.child == "sub-feature" && s.parent == "feature");

    assert!(
      !has_feature_main,
      "Should not suggest feature depends on main (already exists)"
    );
    assert!(has_subfeature_feature, "Should suggest sub-feature depends on feature");

    Ok(())
  }

  #[test]
  fn test_get_auto_discovered_roots() -> Result<()> {
    // Create a temporary git repository
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    // Create main branch explicitly
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Create another independent branch from main
    checkout_branch(repo, "main")?;
    create_branch(repo, "independent", Some("main"))?;
    checkout_branch(repo, "independent")?;
    create_commit(repo, "independent.txt", "Independent content", "Independent commit")?;

    // Initialize repo state
    let repo_state = RepoState::load(repo_path).unwrap_or_default();

    // Run auto dependency discovery to get root branches
    let discovery = AutoDependencyDiscovery;
    let roots = discovery.get_auto_discovered_roots(repo, &repo_state)?;

    // Verify the discovered roots
    assert!(roots.contains(&"main".to_string()), "Main should be a root branch");
    assert!(
      !roots.contains(&"feature".to_string()),
      "Feature should not be a root branch"
    );
    assert!(
      !roots.contains(&"independent".to_string()),
      "Independent should not be a root branch"
    );

    Ok(())
  }

  #[test]
  fn test_with_branch_metadata() -> Result<()> {
    // Create a temporary git repository
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    // Create main branch explicitly
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Add branch metadata
    add_branch_metadata(repo_path, "main", Some("MAIN-123"), None)?;
    add_branch_metadata(repo_path, "feature", Some("FEAT-456"), Some(42))?;

    // Initialize repo state
    let repo_state = RepoState::load(repo_path)?;

    // Run auto dependency discovery
    let discovery = AutoDependencyDiscovery;
    let branch_nodes = discovery.discover_git_dependencies(repo, &repo_state)?;

    // Verify the discovered dependencies with metadata
    assert!(branch_nodes.contains_key("main"));
    assert!(branch_nodes.contains_key("feature"));

    // Check that metadata was properly included
    let main_node = &branch_nodes["main"];
    let feature_node = &branch_nodes["feature"];

    assert!(main_node.metadata.is_some());
    assert_eq!(
      main_node.metadata.as_ref().unwrap().jira_issue.as_deref(),
      Some("MAIN-123")
    );

    assert!(feature_node.metadata.is_some());
    assert_eq!(
      feature_node.metadata.as_ref().unwrap().jira_issue.as_deref(),
      Some("FEAT-456")
    );
    assert_eq!(feature_node.metadata.as_ref().unwrap().github_pr, Some(42));

    Ok(())
  }

  #[test]
  fn test_analyze_commit_ancestry() -> Result<()> {
    // Create a temporary git repository
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let repo = &git_repo.repo;

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    // Create main branch explicitly
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Create sub-feature branch from feature
    create_branch(repo, "sub-feature", Some("feature"))?;
    checkout_branch(repo, "sub-feature")?;
    create_commit(repo, "sub-feature.txt", "Sub-feature content", "Sub-feature commit")?;

    // Create branch info map manually
    let mut branch_info = HashMap::new();

    // Get commit IDs for each branch
    let main_commit = repo
      .find_branch("main", BranchType::Local)?
      .into_reference()
      .peel_to_commit()?
      .id();
    let feature_commit = repo
      .find_branch("feature", BranchType::Local)?
      .into_reference()
      .peel_to_commit()?
      .id();
    let sub_feature_commit = repo
      .find_branch("sub-feature", BranchType::Local)?
      .into_reference()
      .peel_to_commit()?
      .id();

    // Populate branch info
    branch_info.insert("main".to_string(), (main_commit, false));
    branch_info.insert("feature".to_string(), (feature_commit, false));
    branch_info.insert("sub-feature".to_string(), (sub_feature_commit, true));

    // Create branch nodes
    let mut branch_nodes = HashMap::new();
    branch_nodes.insert(
      "main".to_string(),
      BranchNode {
        name: "main".to_string(),
        is_current: false,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    branch_nodes.insert(
      "feature".to_string(),
      BranchNode {
        name: "feature".to_string(),
        is_current: false,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    branch_nodes.insert(
      "sub-feature".to_string(),
      BranchNode {
        name: "sub-feature".to_string(),
        is_current: true,
        metadata: None,
        parents: Vec::new(),
        children: Vec::new(),
      },
    );

    // Create root branches set
    let mut root_branches = branch_nodes
      .keys()
      .cloned()
      .collect::<std::collections::HashSet<String>>();

    // Run analyze_commit_ancestry
    let discovery = AutoDependencyDiscovery;
    discovery.analyze_commit_ancestry(&mut branch_nodes, &branch_info, &mut root_branches, repo)?;

    // Verify the results
    assert!(root_branches.contains("main"), "Main should still be a root branch");
    assert!(
      !root_branches.contains("feature"),
      "Feature should not be a root branch"
    );
    assert!(
      !root_branches.contains("sub-feature"),
      "Sub-feature should not be a root branch"
    );

    // Check parent-child relationships
    let main_node = &branch_nodes["main"];
    let feature_node = &branch_nodes["feature"];
    let sub_feature_node = &branch_nodes["sub-feature"];

    assert!(main_node.children.contains(&"feature".to_string()));
    assert!(feature_node.parents.contains(&"main".to_string()));
    assert!(feature_node.children.contains(&"sub-feature".to_string()));
    assert!(sub_feature_node.parents.contains(&"feature".to_string()));

    Ok(())
  }

  /// Helper function to add a branch-issue association
  pub fn add_branch_metadata(
    repo_path: &Path,
    branch: &str,
    jira_issue: Option<&str>,
    github_pr: Option<u32>,
  ) -> Result<()> {
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();

    let metadata = crate::repo_state::BranchMetadata {
      branch: branch.to_string(),
      jira_issue: jira_issue.map(|s| s.to_string()),
      github_pr,
      created_at: chrono::Utc::now().to_rfc3339(),
    };

    repo_state.add_branch_issue(metadata);
    repo_state.save(repo_path)?;
    Ok(())
  }
}
