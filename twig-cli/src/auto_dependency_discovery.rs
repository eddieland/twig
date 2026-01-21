//! # Auto Dependency Discovery
//!
//! Automatic discovery of Git-based branch dependencies using merge-base
//! distance scoring to suggest parent-child relationships between branches.
//!
//! ## Algorithm
//!
//! For each orphaned branch, we find the best parent among candidate branches
//! using:
//!
//! 1. Find the fork point (merge-base) between child and candidate parent
//! 2. Calculate how far the parent has drifted from the fork point
//! 3. Score = 1.0 / (1.0 + parent_drift)
//! 4. Best parent = highest score (smallest drift)
//!
//! This approach correctly handles the common case where base branches advance
//! while feature work is in progress.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use git2::{BranchType, Oid, Repository as Git2Repository};
use tracing::debug;
use twig_core::RepoState;
use twig_core::tree_renderer::BranchNode;

/// Maximum commits a parent can be ahead of the fork point to still be
/// considered a valid parent candidate. Higher values allow more drift
/// but risk false positives.
const MAX_PARENT_DRIFT: usize = 15;

/// Minimum confidence score to suggest a relationship.
/// Score = 1.0 / (1.0 + parent_drift), so:
///   drift=0 → score=1.0
///   drift=5 → score=0.167
///   drift=15 → score=0.0625
const MIN_CONFIDENCE: f64 = 0.05;

pub struct AutoDependencyDiscovery;

#[derive(Debug, Clone)]
pub struct DependencySuggestion {
  pub child: String,
  pub parent: String,
  pub confidence: f64, // 0.0 to 1.0
  pub reason: String,
}

impl AutoDependencyDiscovery {
  /// Discover Git-based dependencies using commit ancestry with merge-base
  /// scoring
  pub fn discover_git_dependencies(
    &self,
    repo: &Git2Repository,
    repo_state: &RepoState,
  ) -> Result<HashMap<String, BranchNode>> {
    debug!("Starting git dependency discovery");
    let mut branch_nodes = HashMap::new();
    let mut branch_info = HashMap::new();
    let mut root_branches = HashSet::new();
    let mut branch_count = 0usize;

    // Get configured root branches for tiebreaking
    let configured_roots: HashSet<String> = repo_state.get_root_branches().into_iter().collect();

    // Get all local branches
    let branches = repo.branches(Some(BranchType::Local))?;

    // First pass: collect basic branch information
    for branch_result in branches {
      let (branch, _) = branch_result?;
      if let Some(name) = branch.name()? {
        branch_count += 1;
        let is_current = branch.is_head();
        let metadata = repo_state.get_branch_metadata(name).cloned();

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
    debug!(branch_count, "Collected branch information for dependency discovery");

    // Second pass: determine parent-child relationships using merge-base scoring
    self.analyze_commit_ancestry_with_roots(
      &mut branch_nodes,
      &branch_info,
      &mut root_branches,
      repo,
      &configured_roots,
    )?;

    Ok(branch_nodes)
  }

  /// Analyze commit ancestry to determine branch relationships using merge-base
  /// distance scoring.
  ///
  /// For each branch, find the best parent among all other branches using:
  /// 1. Find the fork point (merge-base) between child and candidate parent
  /// 2. Calculate how far the parent has drifted from the fork point
  /// 3. Score = 1.0 / (1.0 + parent_drift)
  /// 4. Best parent = highest score (smallest drift)
  pub fn analyze_commit_ancestry(
    &self,
    branch_nodes: &mut HashMap<String, BranchNode>,
    branch_info: &HashMap<String, (git2::Oid, bool)>,
    root_branches: &mut HashSet<String>,
    repo: &Git2Repository,
  ) -> Result<()> {
    self.analyze_commit_ancestry_with_roots(branch_nodes, branch_info, root_branches, repo, &HashSet::new())
  }

  /// Internal implementation that accepts configured root branches for
  /// tiebreaking
  fn analyze_commit_ancestry_with_roots(
    &self,
    branch_nodes: &mut HashMap<String, BranchNode>,
    branch_info: &HashMap<String, (Oid, bool)>,
    root_branches: &mut HashSet<String>,
    repo: &Git2Repository,
    configured_roots: &HashSet<String>,
  ) -> Result<()> {
    let total = branch_info.len();
    debug!(total, "Starting merge-base ancestry analysis");

    // Track which branches have children (for tiebreaking)
    let mut branches_with_children: HashSet<String> = HashSet::new();

    // First pass: find best parent for each branch
    let mut relationships: Vec<(String, String, f64)> = Vec::new(); // (child, parent, confidence)

    for (idx, (branch_name, (commit_id, _))) in branch_info.iter().enumerate() {
      if idx % 25 == 0 || idx + 1 == total {
        debug!(
          processed = idx + 1,
          total,
          branch = branch_name,
          "Analyzing branch ancestry with merge-base"
        );
      }

      // Find the best parent for this branch
      if let Some((best_parent, confidence)) = self.find_best_parent(
        repo,
        *commit_id,
        branch_name,
        branch_info,
        configured_roots,
        &branches_with_children,
      ) {
        relationships.push((branch_name.clone(), best_parent.clone(), confidence));
        branches_with_children.insert(best_parent);
      }
    }

    // Apply relationships to branch nodes
    for (child_name, parent_name, _confidence) in &relationships {
      // Add parent to child's parents list
      if let Some(child_node) = branch_nodes.get_mut(child_name) {
        child_node.parents.push(parent_name.clone());
      }

      // Add child to parent's children list
      if let Some(parent_node) = branch_nodes.get_mut(parent_name) {
        parent_node.children.push(child_name.clone());
      }

      // This branch is no longer a root branch
      root_branches.remove(child_name);
    }

    // If no root branches were found, use the current branch or the first branch as
    // root
    if root_branches.is_empty() {
      let root_branch = branch_info
        .iter()
        .find(|(_, (_, is_current))| *is_current)
        .map(|(name, _)| name.clone())
        .or_else(|| branch_info.keys().next().cloned());

      if let Some(name) = root_branch {
        root_branches.insert(name);
      }
    }

    debug!(
      root_count = root_branches.len(),
      relationships = relationships.len(),
      "Finished merge-base ancestry analysis"
    );
    Ok(())
  }

  /// Find the best parent for a branch using merge-base distance scoring.
  ///
  /// Returns the best parent name and confidence score, or None if no valid
  /// parent found.
  fn find_best_parent(
    &self,
    repo: &Git2Repository,
    child_oid: Oid,
    child_name: &str,
    candidates: &HashMap<String, (Oid, bool)>,
    configured_roots: &HashSet<String>,
    _branches_with_children: &HashSet<String>,
  ) -> Option<(String, f64)> {
    // (name, score, parent_drift, child_depth)
    let mut best_candidates: Vec<(String, f64, usize, usize)> = Vec::new();

    for (candidate_name, (candidate_oid, _)) in candidates {
      if candidate_name == child_name {
        continue; // Skip self
      }

      // Find the fork point (merge-base)
      let Ok(fork_point) = repo.merge_base(child_oid, *candidate_oid) else {
        continue; // No common ancestor
      };

      // CRITICAL: Verify that the candidate is actually an ancestor of the child.
      // This prevents sibling branches (which share a fork point but neither is
      // an ancestor of the other) from being suggested as parents.
      //
      // graph_descendant_of(child, candidate) returns true if candidate is an
      // ancestor of child.
      let is_ancestor = repo.graph_descendant_of(child_oid, *candidate_oid).unwrap_or(false);

      // If the candidate is not a direct ancestor, it might be:
      // 1. A sibling branch (both branched from the same point) - REJECT
      // 2. A parent branch that has advanced since the child branched - ALLOW if root
      //
      // We only allow non-ancestor candidates if they are configured root branches,
      // as these are explicitly designated as base branches for the repository.
      if !is_ancestor && !configured_roots.contains(candidate_name) {
        debug!(
          child = child_name,
          candidate = candidate_name,
          "Rejecting non-ancestor candidate (likely a sibling branch)"
        );
        continue;
      }

      // Calculate how far the child is from the fork point (child_depth)
      // This tells us how many commits the child has made since branching off
      let Ok((child_depth, _)) = repo.graph_ahead_behind(child_oid, fork_point) else {
        continue;
      };

      // The child must have at least 1 commit beyond the fork point
      // Otherwise this isn't a valid parent-child relationship (the "child" hasn't
      // actually branched)
      if child_depth == 0 {
        continue;
      }

      // Calculate how far the parent has drifted from the fork point
      // graph_ahead_behind returns (ahead, behind) where:
      // - ahead = commits in candidate_oid not in fork_point
      // - behind = commits in fork_point not in candidate_oid (always 0 since
      //   fork_point is ancestor)
      let Ok((parent_drift, _)) = repo.graph_ahead_behind(*candidate_oid, fork_point) else {
        continue;
      };

      // Reject if parent has drifted too far
      if parent_drift > MAX_PARENT_DRIFT {
        debug!(
          child = child_name,
          parent = candidate_name,
          drift = parent_drift,
          max = MAX_PARENT_DRIFT,
          "Rejecting parent due to excessive drift"
        );
        continue;
      }

      let score = 1.0 / (1.0 + parent_drift as f64);

      if score >= MIN_CONFIDENCE {
        best_candidates.push((candidate_name.clone(), score, parent_drift, child_depth));
      }
    }

    if best_candidates.is_empty() {
      return None;
    }

    // Sort by tiebreaking criteria:
    // 1. Higher score (smaller parent drift) first
    // 2. Smaller child_depth (prefer more direct branching relationship)
    // 3. Prefer configured root branches
    // 4. Alphabetical (deterministic fallback)
    best_candidates.sort_by(|a, b| {
      // Compare scores (higher is better, meaning smaller parent drift)
      let score_cmp = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
      if score_cmp != std::cmp::Ordering::Equal {
        return score_cmp;
      }

      // Prefer smaller child_depth (more direct relationship)
      // A smaller child_depth means the child branched more directly from this parent
      let depth_cmp = a.3.cmp(&b.3);
      if depth_cmp != std::cmp::Ordering::Equal {
        return depth_cmp;
      }

      // Prefer configured root branches
      let a_is_root = configured_roots.contains(&a.0);
      let b_is_root = configured_roots.contains(&b.0);
      if a_is_root != b_is_root {
        return if b_is_root {
          std::cmp::Ordering::Greater
        } else {
          std::cmp::Ordering::Less
        };
      }

      // Alphabetical fallback
      a.0.cmp(&b.0)
    });

    let (name, score, _drift, _depth) = best_candidates.into_iter().next()?;
    Some((name, score))
  }

  /// Suggest dependencies based on Git history analysis with merge-base scoring
  pub fn suggest_dependencies(
    &self,
    repo: &Git2Repository,
    repo_state: &RepoState,
  ) -> Result<Vec<DependencySuggestion>> {
    debug!("Computing dependency suggestions");
    let mut suggestions = Vec::new();
    let mut branch_info = HashMap::new();

    // Get configured root branches for tiebreaking
    let configured_roots: HashSet<String> = repo_state.get_root_branches().into_iter().collect();

    // Get all local branches
    let branches = repo.branches(Some(BranchType::Local))?;
    for branch_result in branches {
      let (branch, _) = branch_result?;
      if let Some(name) = branch.name()? {
        let is_current = branch.is_head();
        let commit = branch.get().peel_to_commit()?;
        branch_info.insert(name.to_string(), (commit.id(), is_current));
      }
    }

    // Track which branches have children (for tiebreaking)
    let mut branches_with_children: HashSet<String> = HashSet::new();

    // Find best parent for each branch and compute confidence
    for (branch_name, (commit_id, _)) in &branch_info {
      if let Some((best_parent, confidence)) = self.find_best_parent(
        repo,
        *commit_id,
        branch_name,
        &branch_info,
        &configured_roots,
        &branches_with_children,
      ) {
        // Skip if this dependency already exists in user-defined dependencies
        let already_exists = repo_state
          .list_dependencies()
          .iter()
          .any(|d| d.child == *branch_name && d.parent == best_parent);

        if !already_exists {
          let drift_description = if confidence >= 1.0 {
            "parent branch unchanged since fork".to_string()
          } else if confidence >= 0.5 {
            "parent branch has minimal drift from fork point".to_string()
          } else {
            "parent branch has moderate drift from fork point".to_string()
          };

          suggestions.push(DependencySuggestion {
            child: branch_name.clone(),
            parent: best_parent.clone(),
            confidence,
            reason: format!("Based on merge-base analysis: {}", drift_description),
          });
          branches_with_children.insert(best_parent);
        }
      }
    }

    debug!(
      branch_count = branch_info.len(),
      suggestion_count = suggestions.len(),
      "Dependency suggestions computed"
    );
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

  use twig_core::state::BranchMetadata;
  use twig_core::tree_renderer::BranchNode;
  use twig_test_utils::git::{GitRepoTestGuard, checkout_branch, create_branch, create_commit, ensure_main_branch};

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
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    ensure_main_branch(repo)?;

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
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    ensure_main_branch(repo)?;

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
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    ensure_main_branch(repo)?;

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
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    ensure_main_branch(repo)?;

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
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    ensure_main_branch(repo)?;

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
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;

    ensure_main_branch(repo)?;

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

    let metadata = BranchMetadata {
      branch: branch.to_string(),
      jira_issue: jira_issue.map(|s| s.to_string()),
      github_pr,
      created_at: chrono::Utc::now().to_rfc3339(),
    };

    repo_state.add_branch_issue(metadata);
    repo_state.save(repo_path)?;
    Ok(())
  }

  // ============================================================================
  // Conservative Auto-Adopt Tests (merge-base distance scoring)
  // ============================================================================

  /// Test: Branch cut from main, main unchanged → suggests main as parent with
  /// high confidence
  #[test]
  fn test_accepts_direct_child() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main branch
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create feature branch directly from main (no drift)
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    let repo_state = RepoState::load(repo_path).unwrap_or_default();
    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Should suggest feature -> main with high confidence (score = 1.0 for drift=0)
    let suggestion = suggestions.iter().find(|s| s.child == "feature" && s.parent == "main");

    assert!(suggestion.is_some(), "Should suggest feature depends on main");
    assert!(
      suggestion.expect("checked above").confidence >= 1.0,
      "Confidence should be 1.0 when parent has not drifted"
    );

    Ok(())
  }

  /// Test: Branch cut from main, main advanced 5 commits → still suggests main
  /// (when main is configured as a root branch)
  #[test]
  fn test_accepts_drifted_base() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create feature branch from main
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Now advance main by 5 commits (simulating other work being merged)
    checkout_branch(repo, "main")?;
    for i in 1..=5 {
      create_commit(
        repo,
        &format!("main_file_{}.txt", i),
        &format!("Content {}", i),
        &format!("Main commit {}", i),
      )?;
    }

    // Configure main as a root branch (required for non-ancestor parent detection)
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
    let _ = repo_state.add_root("main".to_string(), true);
    repo_state.save(repo_path)?;
    let repo_state = RepoState::load(repo_path)?;

    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Should still suggest feature -> main (drift of 5 is within MAX_PARENT_DRIFT)
    let suggestion = suggestions.iter().find(|s| s.child == "feature" && s.parent == "main");

    assert!(
      suggestion.is_some(),
      "Should suggest feature depends on main even with drift"
    );
    let confidence = suggestion.expect("checked above").confidence;
    // confidence = 1.0 / (1.0 + 5) = ~0.167
    assert!(
      confidence > 0.1 && confidence < 1.0,
      "Confidence should be reduced but still valid (got {})",
      confidence
    );

    Ok(())
  }

  /// Test: Branch cut from main 100+ commits ago → no suggestion (exceeds
  /// MAX_PARENT_DRIFT)
  #[test]
  fn test_rejects_ancient_ancestor() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create old feature branch from main
    create_branch(repo, "feature-old", Some("main"))?;
    checkout_branch(repo, "feature-old")?;
    create_commit(repo, "old_feature.txt", "Old feature", "Old feature commit")?;

    // Now advance main by many commits (more than MAX_PARENT_DRIFT = 15)
    checkout_branch(repo, "main")?;
    for i in 1..=20 {
      create_commit(
        repo,
        &format!("main_file_{}.txt", i),
        &format!("Content {}", i),
        &format!("Main commit {}", i),
      )?;
    }

    let repo_state = RepoState::load(repo_path).unwrap_or_default();
    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Should NOT suggest feature-old -> main (drift of 20 exceeds MAX_PARENT_DRIFT
    // = 15)
    let suggestion = suggestions
      .iter()
      .find(|s| s.child == "feature-old" && s.parent == "main");

    assert!(
      suggestion.is_none(),
      "Should not suggest feature-old depends on main when drift exceeds threshold"
    );

    Ok(())
  }

  /// Test: When both main and develop are valid, prefers the one with smaller
  /// drift (both must be configured as root branches since they've advanced)
  #[test]
  fn test_prefers_closest_parent() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create develop branch from main with some commits
    create_branch(repo, "develop", Some("main"))?;
    checkout_branch(repo, "develop")?;
    create_commit(repo, "develop1.txt", "Develop 1", "Develop commit 1")?;
    create_commit(repo, "develop2.txt", "Develop 2", "Develop commit 2")?;

    // Create feature branch from develop
    create_branch(repo, "feature", Some("develop"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Advance main by a few commits (so both main and develop could be "parents")
    checkout_branch(repo, "main")?;
    for i in 1..=3 {
      create_commit(
        repo,
        &format!("main_{}.txt", i),
        &format!("Main {}", i),
        &format!("Main commit {}", i),
      )?;
    }

    // Advance develop by just 1 commit
    checkout_branch(repo, "develop")?;
    create_commit(repo, "develop3.txt", "Develop 3", "Develop commit 3")?;

    // Configure both main and develop as root branches (required for non-ancestor
    // detection)
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
    let _ = repo_state.add_root("main".to_string(), true);
    let _ = repo_state.add_root("develop".to_string(), false);
    repo_state.save(repo_path)?;
    let repo_state = RepoState::load(repo_path)?;

    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Should suggest feature -> develop (develop has smaller drift = 1, main has
    // drift = 3)
    let suggestion = suggestions.iter().find(|s| s.child == "feature");

    assert!(suggestion.is_some(), "Should have a suggestion for feature");
    assert_eq!(
      suggestion.expect("checked above").parent,
      "develop",
      "Should prefer develop (smaller drift) over main"
    );

    Ok(())
  }

  /// Test: Disjoint histories → no suggestion, falls back gracefully
  #[test]
  fn test_handles_no_common_ancestor() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create an orphan branch (no common ancestor with main)
    // git checkout --orphan creates a branch with no parent commit
    let orphan_oid = {
      let tree_id = {
        let mut index = repo.index()?;
        let file_path = repo.workdir().expect("should have workdir").join("orphan.txt");
        std::fs::write(&file_path, "Orphan content")?;
        index.add_path(std::path::Path::new("orphan.txt"))?;
        index.write_tree()?
      };
      let tree = repo.find_tree(tree_id)?;
      let sig = git2::Signature::now("Test", "test@example.com")?;
      repo.commit(None, &sig, &sig, "Orphan commit", &tree, &[])?
    };

    // Create the orphan branch reference
    repo.branch("orphan-branch", &repo.find_commit(orphan_oid)?, false)?;

    let repo_state = RepoState::load(repo_path).unwrap_or_default();
    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Should NOT suggest orphan-branch -> main (no common ancestor)
    let suggestion = suggestions.iter().find(|s| s.child == "orphan-branch");

    assert!(
      suggestion.is_none(),
      "Should not suggest parent for orphan branch with no common ancestor"
    );

    Ok(())
  }

  /// Test: Verify confidence scoring formula
  #[test]
  fn test_confidence_scoring() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create feature branch
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Advance main by 10 commits
    checkout_branch(repo, "main")?;
    for i in 1..=10 {
      create_commit(
        repo,
        &format!("main_{}.txt", i),
        &format!("Main {}", i),
        &format!("Main commit {}", i),
      )?;
    }

    // Configure main as a root branch (required for non-ancestor detection)
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
    let _ = repo_state.add_root("main".to_string(), true);
    repo_state.save(repo_path)?;
    let repo_state = RepoState::load(repo_path)?;

    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    let suggestion = suggestions.iter().find(|s| s.child == "feature" && s.parent == "main");
    assert!(suggestion.is_some(), "Should suggest feature depends on main");

    // Expected confidence: 1.0 / (1.0 + 10) = ~0.0909
    let confidence = suggestion.expect("checked above").confidence;
    let expected = 1.0 / (1.0 + 10.0);
    let tolerance = 0.001;
    assert!(
      (confidence - expected).abs() < tolerance,
      "Confidence should be ~{:.4}, got {:.4}",
      expected,
      confidence
    );

    Ok(())
  }

  /// Test: Root branches are preferred in tiebreaking
  #[test]
  fn test_prefers_configured_root_branches() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create "alpha" branch from main (alphabetically before "main")
    create_branch(repo, "alpha", Some("main"))?;

    // Create feature branch from main (could match either main or alpha since they
    // point to same commit)
    checkout_branch(repo, "main")?;
    create_branch(repo, "feature", Some("main"))?;
    checkout_branch(repo, "feature")?;
    create_commit(repo, "feature.txt", "Feature content", "Feature commit")?;

    // Configure main as a root branch
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
    let _ = repo_state.add_root("main".to_string(), true);
    repo_state.save(repo_path)?;
    let repo_state = RepoState::load(repo_path)?;

    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // Should prefer main over alpha due to tiebreaking (main is configured root)
    let suggestion = suggestions.iter().find(|s| s.child == "feature");
    assert!(suggestion.is_some(), "Should have suggestion for feature");
    assert_eq!(
      suggestion.expect("checked above").parent,
      "main",
      "Should prefer configured root branch (main) in tiebreaking"
    );

    Ok(())
  }

  /// Test: Sibling branches should NOT be suggested as parents, even when they
  /// have smaller drift than the actual parent.
  ///
  /// Scenario:
  /// - main has advanced by 5 commits after feature-A and feature-B branched
  /// - feature-A has 1 commit (drift = 1)
  /// - feature-B has 1 commit
  /// - Without ancestry check: feature-A would win (smaller drift)
  /// - With ancestry check: feature-A is rejected (not an ancestor of
  ///   feature-B)
  #[test]
  fn test_rejects_sibling_branches() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create feature-A from main (this will be the sibling)
    create_branch(repo, "feature-a", Some("main"))?;
    checkout_branch(repo, "feature-a")?;
    create_commit(repo, "feature_a.txt", "Feature A content", "Feature A commit")?;

    // Go back to main and create feature-B (also from main, making it a sibling of
    // feature-A)
    checkout_branch(repo, "main")?;
    create_branch(repo, "feature-b", Some("main"))?;
    checkout_branch(repo, "feature-b")?;
    create_commit(repo, "feature_b.txt", "Feature B content", "Feature B commit")?;

    // Now advance main by 5 commits (more than feature-A's 1 commit)
    // This makes main's drift (5) greater than feature-A's drift (1)
    checkout_branch(repo, "main")?;
    for i in 1..=5 {
      create_commit(
        repo,
        &format!("main_{}.txt", i),
        &format!("Main {}", i),
        &format!("Main commit {}", i),
      )?;
    }

    // Configure main as a root branch (required for non-ancestor parents)
    let mut repo_state = RepoState::load(repo_path).unwrap_or_default();
    let _ = repo_state.add_root("main".to_string(), true);
    repo_state.save(repo_path)?;
    let repo_state = RepoState::load(repo_path)?;

    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // feature-B should get main as parent, NOT feature-A
    // Even though feature-A has smaller drift (1) vs main (5), feature-A is a
    // sibling
    let suggestion = suggestions.iter().find(|s| s.child == "feature-b");
    assert!(suggestion.is_some(), "Should have suggestion for feature-b");
    assert_eq!(
      suggestion.expect("checked above").parent,
      "main",
      "Should suggest main (configured root) as parent, not sibling feature-a"
    );

    // feature-A should also get main as parent
    let suggestion_a = suggestions.iter().find(|s| s.child == "feature-a");
    assert!(suggestion_a.is_some(), "Should have suggestion for feature-a");
    assert_eq!(
      suggestion_a.expect("checked above").parent,
      "main",
      "Should suggest main as parent for feature-a too"
    );

    Ok(())
  }

  /// Test: Without configured root branches, sibling branches with smaller
  /// drift should not be suggested (they're not ancestors)
  #[test]
  fn test_sibling_rejection_without_root_config() -> Result<()> {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit on main
    create_commit(repo, "file1.txt", "Initial content", "Initial commit")?;
    ensure_main_branch(repo)?;

    // Create feature-A from main
    create_branch(repo, "feature-a", Some("main"))?;
    checkout_branch(repo, "feature-a")?;
    create_commit(repo, "feature_a.txt", "Feature A content", "Feature A commit")?;

    // Create feature-B from main (sibling of feature-A)
    checkout_branch(repo, "main")?;
    create_branch(repo, "feature-b", Some("main"))?;
    checkout_branch(repo, "feature-b")?;
    create_commit(repo, "feature_b.txt", "Feature B content", "Feature B commit")?;

    // Advance main by many commits (beyond MAX_PARENT_DRIFT threshold)
    checkout_branch(repo, "main")?;
    for i in 1..=20 {
      create_commit(
        repo,
        &format!("main_{}.txt", i),
        &format!("Main {}", i),
        &format!("Main commit {}", i),
      )?;
    }

    // NO root branches configured
    let repo_state = RepoState::load(repo_path).unwrap_or_default();

    let discovery = AutoDependencyDiscovery;
    let suggestions = discovery.suggest_dependencies(repo, &repo_state)?;

    // feature-B should NOT have feature-A suggested as parent
    // (feature-A is not an ancestor, and no root branches are configured)
    // main is also rejected due to excessive drift (20 > MAX_PARENT_DRIFT)
    let suggestion = suggestions.iter().find(|s| s.child == "feature-b");

    // With no valid parent candidates, there should be no suggestion
    assert!(
      suggestion.is_none() || suggestion.expect("is some").parent != "feature-a",
      "Should NOT suggest sibling feature-a as parent of feature-b"
    );

    Ok(())
  }
}
