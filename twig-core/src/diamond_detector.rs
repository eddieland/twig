//! # Diamond Pattern Detection
//!
//! Detects diamond dependency patterns in branch trees where multiple paths 
//! converge at a common merge point after diverging from a common ancestor.

use std::collections::{HashMap, HashSet};
use crate::tree_renderer::BranchNode;

/// Represents a diamond pattern in the dependency tree
#[derive(Debug, Clone)]
pub struct DiamondPattern {
  /// The common ancestor branch (top of diamond)
  pub ancestor: String,
  /// The merge point branch (bottom of diamond)
  pub merge_point: String,
  /// The left path branches (excluding ancestor and merge point)
  pub left_path: Vec<String>,
  /// The right path branches (excluding ancestor and merge point)  
  pub right_path: Vec<String>,
  /// Depth of the diamond (distance from ancestor to merge point)
  pub depth: u32,
  /// Whether this diamond is nested within another
  pub is_nested: bool,
}

/// Diamond pattern detector
pub struct DiamondDetector<'a> {
  branch_nodes: &'a HashMap<String, BranchNode>,
}

impl<'a> DiamondDetector<'a> {
  /// Create a new diamond detector
  pub fn new(branch_nodes: &'a HashMap<String, BranchNode>) -> Self {
    Self { branch_nodes }
  }

  /// Detect all diamond patterns in the branch tree
  pub fn detect_diamond_patterns(&self) -> Vec<DiamondPattern> {
    let mut patterns = Vec::new();
    let mut visited_diamonds: HashSet<String> = HashSet::new();
    
    // Find all branches with multiple parents (potential merge points)
    for (branch_name, node) in self.branch_nodes {
      if node.parents.len() >= 2 && !visited_diamonds.contains(branch_name) {
        if let Some(diamond) = self.analyze_diamond_pattern(branch_name, &mut visited_diamonds) {
          patterns.push(diamond);
        }
      }
    }
    
    // Sort diamonds by depth for proper nesting detection
    patterns.sort_by_key(|d| d.depth);
    
    // Mark nested diamonds
    self.mark_nested_diamonds(&mut patterns);
    
    patterns
  }

  /// Analyze if a branch with multiple parents forms a diamond pattern
  fn analyze_diamond_pattern(
    &self,
    merge_point: &str,
    visited_diamonds: &mut HashSet<String>,
  ) -> Option<DiamondPattern> {
    let merge_node = self.branch_nodes.get(merge_point)?;
    
    if merge_node.parents.len() < 2 {
      return None;
    }

    // Find common ancestors of the parent branches
    let common_ancestors = self.find_common_ancestors(&merge_node.parents);
    
    for ancestor in common_ancestors {
      // Check if we can form a valid diamond
      if let Some(diamond) = self.construct_diamond(&ancestor, merge_point, &merge_node.parents) {
        visited_diamonds.insert(merge_point.to_string());
        return Some(diamond);
      }
    }
    
    None
  }

  /// Find common ancestors of a set of branches
  fn find_common_ancestors(&self, branches: &[String]) -> Vec<String> {
    if branches.len() < 2 {
      return Vec::new();
    }

    // Get all ancestors for the first branch
    let mut common_ancestors: HashSet<String> = self.get_all_ancestors(&branches[0]);
    
    // Intersect with ancestors of other branches
    for branch in &branches[1..] {
      let ancestors = self.get_all_ancestors(branch);
      common_ancestors = common_ancestors.intersection(&ancestors).cloned().collect();
    }
    
    // Filter to find the most recent common ancestors (closest to merge point)
    let mut result: Vec<String> = common_ancestors.into_iter().collect();
    result.sort_by_key(|ancestor| self.calculate_distance_to_merge(ancestor, &branches[0]));
    
    result
  }

  /// Get all ancestors of a branch (recursive parent traversal)
  fn get_all_ancestors(&self, branch: &str) -> HashSet<String> {
    let mut ancestors = HashSet::new();
    let mut to_visit = vec![branch.to_string()];
    let mut visited = HashSet::new();
    
    while let Some(current) = to_visit.pop() {
      if visited.contains(&current) {
        continue;
      }
      visited.insert(current.clone());
      
      if let Some(node) = self.branch_nodes.get(&current) {
        for parent in &node.parents {
          ancestors.insert(parent.clone());
          to_visit.push(parent.clone());
        }
      }
    }
    
    ancestors
  }

  /// Calculate approximate distance between two branches
  fn calculate_distance_to_merge(&self, from: &str, to: &str) -> u32 {
    let mut distance = 0;
    let mut current = to;
    let mut visited = HashSet::new();
    
    while let Some(node) = self.branch_nodes.get(current) {
      if visited.contains(current) || current == from {
        break;
      }
      visited.insert(current.to_string());
      
      distance += 1;
      if distance > 20 {
        // Prevent infinite loops in complex graphs
        break;
      }
      
      // Move to first parent (simplified traversal)
      if let Some(first_parent) = node.parents.first() {
        current = first_parent;
      } else {
        break;
      }
    }
    
    distance
  }

  /// Construct a diamond pattern from ancestor and merge point
  fn construct_diamond(
    &self,
    ancestor: &str,
    merge_point: &str,
    merge_parents: &[String],
  ) -> Option<DiamondPattern> {
    // Find paths from ancestor to merge point through different parents
    let mut paths = Vec::new();
    
    for parent in merge_parents {
      if let Some(path) = self.find_path_excluding_endpoints(ancestor, parent) {
        paths.push(path);
      }
    }
    
    // We need at least 2 paths to form a diamond
    // For simple diamonds, the different merge_parents themselves represent different paths
    if paths.len() >= 2 && merge_parents.len() >= 2 {
      let depth = paths.iter().map(|p| p.len() as u32).max().unwrap_or(0) + 2; // +2 for ancestor and merge point
      
      Some(DiamondPattern {
        ancestor: ancestor.to_string(),
        merge_point: merge_point.to_string(),
        left_path: paths.get(0).cloned().unwrap_or_default(),
        right_path: paths.get(1).cloned().unwrap_or_default(),
        depth,
        is_nested: false, // Will be determined later
      })
    } else {
      None
    }
  }

  /// Find path between two branches excluding the endpoints
  fn find_path_excluding_endpoints(&self, from: &str, to: &str) -> Option<Vec<String>> {
    let mut path = Vec::new();
    let mut current = to;
    let mut visited = HashSet::new();
    
    while let Some(node) = self.branch_nodes.get(current) {
      if visited.contains(current) {
        break; // Prevent infinite loops
      }
      visited.insert(current.to_string());
      
      if current == from {
        return Some(path);
      }
      
      // Add current to path (excluding the target)
      if current != to {
        path.insert(0, current.to_string());
      }
      
      if path.len() > 15 {
        // Prevent overly long paths
        break;
      }
      
      // Move to first parent
      if let Some(first_parent) = node.parents.first() {
        current = first_parent;
      } else {
        break;
      }
    }
    
    None
  }

  /// Mark diamonds that are nested within other diamonds
  fn mark_nested_diamonds(&self, patterns: &mut [DiamondPattern]) {
    let pattern_count = patterns.len();
    
    for i in 0..pattern_count {
      for j in 0..pattern_count {
        if i != j {
          let (outer_depth, inner_depth) = {
            let outer = &patterns[j];
            let inner = &patterns[i];
            (outer.depth, inner.depth)
          };
          
          // If inner diamond is smaller and could be contained within outer
          if inner_depth < outer_depth {
            let is_contained = {
              let outer = &patterns[j];
              let inner = &patterns[i];
              
              // Check if inner diamond's branches are within outer diamond's scope
              self.is_diamond_contained(inner, outer)
            };
            
            if is_contained {
              patterns[i].is_nested = true;
            }
          }
        }
      }
    }
  }

  /// Check if one diamond pattern is contained within another
  fn is_diamond_contained(&self, inner: &DiamondPattern, outer: &DiamondPattern) -> bool {
    // Simplified containment check: inner diamond's merge point should be reachable
    // from outer diamond's ancestor and inner's ancestor should be reachable from outer's ancestor
    
    let inner_in_outer_scope = self.is_branch_reachable(&outer.ancestor, &inner.ancestor) &&
                               self.is_branch_reachable(&inner.merge_point, &outer.merge_point);
    
    inner_in_outer_scope
  }

  /// Check if target branch is reachable from source branch
  fn is_branch_reachable(&self, source: &str, target: &str) -> bool {
    if source == target {
      return true;
    }
    
    let mut to_visit = vec![source.to_string()];
    let mut visited = HashSet::new();
    
    while let Some(current) = to_visit.pop() {
      if visited.contains(&current) {
        continue;
      }
      visited.insert(current.clone());
      
      if current == target {
        return true;
      }
      
      if visited.len() > 50 {
        // Prevent excessive traversal
        break;
      }
      
      if let Some(node) = self.branch_nodes.get(&current) {
        for child in &node.children {
          to_visit.push(child.clone());
        }
      }
    }
    
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tree_renderer::BranchNode;
  use crate::state::BranchMetadata;

  fn create_test_branch(name: &str, is_current: bool, parents: Vec<String>, children: Vec<String>) -> BranchNode {
    BranchNode {
      name: name.to_string(),
      is_current,
      metadata: Some(BranchMetadata {
        branch: name.to_string(),
        jira_issue: None,
        github_pr: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
      }),
      parents,
      children,
    }
  }

  #[test]
  fn test_simple_diamond_detection() {
    let mut branches = HashMap::new();
    
    // Create a simple diamond: main -> feature1, feature2 -> merge
    branches.insert("main".to_string(), create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]));
    branches.insert("feature1".to_string(), create_test_branch("feature1", false, vec!["main".to_string()], vec!["merge".to_string()]));
    branches.insert("feature2".to_string(), create_test_branch("feature2", false, vec!["main".to_string()], vec!["merge".to_string()]));
    branches.insert("merge".to_string(), create_test_branch("merge", false, vec!["feature1".to_string(), "feature2".to_string()], vec![]));

    let detector = DiamondDetector::new(&branches);
    let diamonds = detector.detect_diamond_patterns();

    assert_eq!(diamonds.len(), 1, "Expected exactly 1 diamond pattern");
    let diamond = &diamonds[0];
    assert_eq!(diamond.ancestor, "main");
    assert_eq!(diamond.merge_point, "merge");
    assert_eq!(diamond.depth, 2);
    assert!(!diamond.is_nested);
  }

  #[test]
  fn test_no_diamond_linear_chain() {
    let mut branches = HashMap::new();
    
    // Create a linear chain: main -> feature1 -> feature2
    branches.insert("main".to_string(), create_test_branch("main", false, vec![], vec!["feature1".to_string()]));
    branches.insert("feature1".to_string(), create_test_branch("feature1", false, vec!["main".to_string()], vec!["feature2".to_string()]));
    branches.insert("feature2".to_string(), create_test_branch("feature2", false, vec!["feature1".to_string()], vec![]));

    let detector = DiamondDetector::new(&branches);
    let diamonds = detector.detect_diamond_patterns();

    assert_eq!(diamonds.len(), 0);
  }

  #[test]
  fn test_nested_diamond_detection() {
    let mut branches = HashMap::new();
    
    // Create nested diamonds: main -> feature1, feature2 -> inner_merge -> outer_merge
    branches.insert("main".to_string(), create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]));
    branches.insert("feature1".to_string(), create_test_branch("feature1", false, vec!["main".to_string()], vec!["inner_merge".to_string()]));
    branches.insert("feature2".to_string(), create_test_branch("feature2", false, vec!["main".to_string()], vec!["inner_merge".to_string()]));
    branches.insert("inner_merge".to_string(), create_test_branch("inner_merge", false, vec!["feature1".to_string(), "feature2".to_string()], vec!["outer_feature".to_string()]));
    branches.insert("outer_feature".to_string(), create_test_branch("outer_feature", false, vec!["inner_merge".to_string()], vec!["outer_merge".to_string()]));
    branches.insert("outer_merge".to_string(), create_test_branch("outer_merge", false, vec!["inner_merge".to_string(), "outer_feature".to_string()], vec![]));

    let detector = DiamondDetector::new(&branches);
    let diamonds = detector.detect_diamond_patterns();

    assert!(diamonds.len() >= 1);
    // Verify at least one diamond was detected
    let has_main_diamond = diamonds.iter().any(|d| d.ancestor == "main" && d.merge_point == "inner_merge");
    assert!(has_main_diamond);
  }
}