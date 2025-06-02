//! # Branch Tree Renderer
//!
//! Provides tree visualization and rendering functionality for Git branch
//! dependencies, including formatting, coloring, and hierarchical display.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use owo_colors::OwoColorize;

use crate::repo_state::BranchMetadata;

/// Represents a branch node in the tree
#[derive(Debug, Clone)]
pub struct BranchNode {
  pub name: String,
  pub is_current: bool,
  pub metadata: Option<BranchMetadata>,
  pub parents: Vec<String>,
  pub children: Vec<String>,
}

/// Renderer for the branch tree
pub struct TreeRenderer<'a> {
  pub branch_nodes: &'a HashMap<String, BranchNode>,
  pub visited: HashSet<String>,
  pub cross_refs: HashMap<String, Vec<String>>,
  pub max_depth: Option<u32>,
  pub no_color: bool,
  pub tree_width: usize, // Add field to store calculated tree width
}

impl<'a> TreeRenderer<'a> {
  /// Build cross-references for branches that appear in multiple locations
  pub fn build_cross_references(&mut self) {
    // For each branch, find all its parents
    for (name, node) in self.branch_nodes {
      if node.parents.len() > 1 {
        // This branch has multiple parents, so it will appear in multiple places
        self.cross_refs.insert(name.clone(), node.parents.clone());
      }
    }
  }

  /// Calculate the maximum width of the tree structure (including indentation
  /// and branch names)
  pub fn calculate_max_tree_width(&self, roots: &[String]) -> usize {
    let mut max_width = 0;
    let mut visited = HashSet::new();

    // Process each root branch
    for root in roots {
      let width = self.calculate_branch_width(root, 0, &Vec::new(), &mut visited);
      max_width = max_width.max(width);
    }

    // Add some padding
    max_width + 2
  }

  /// Calculate the width of a branch and its children
  fn calculate_branch_width(
    &self,
    branch_name: &str,
    depth: u32,
    prefix: &[String],
    visited: &mut HashSet<String>,
  ) -> usize {
    // Check max depth
    if let Some(max_depth) = self.max_depth {
      if depth > max_depth {
        return 0;
      }
    }

    // Check if we've already visited this branch
    if visited.contains(branch_name) {
      return 0;
    }

    // Mark as visited
    visited.insert(branch_name.to_string());

    // Get the branch node
    let node = match self.branch_nodes.get(branch_name) {
      Some(node) => node,
      None => return 0,
    };

    // Calculate the width of this branch
    let mut line_width = 0;

    // Add the prefix width
    for p in prefix {
      line_width += p.chars().count();
    }

    // Add the branch symbol width
    if depth > 0 {
      line_width += 4; // "├── " or "└── "
    }

    // Add the branch name width (without color codes for width calculation)
    let branch_display = if node.is_current {
      format!("{} (current)", node.name)
    } else {
      node.name.clone()
    };
    line_width += branch_display.chars().count();

    let mut max_width = line_width;

    // Calculate width for children
    let children = node.children.clone();
    let child_count = children.len();

    for (i, child) in children.iter().enumerate() {
      let is_last = i == child_count - 1;

      // Create a new prefix for the child
      let mut new_prefix = prefix.to_vec();
      if depth > 0 {
        new_prefix.push(if is_last {
          "    ".to_string()
        } else {
          "│   ".to_string()
        });
      }

      // Calculate child width
      let child_width = self.calculate_branch_width(child, depth + 1, &new_prefix, visited);
      max_width = max_width.max(child_width);
    }

    max_width
  }

  /// Get display width of a string, accounting for ANSI color codes
  fn display_width(&self, s: &str) -> usize {
    if self.no_color {
      s.chars().count()
    } else {
      // Strip ANSI codes for width calculation
      console::strip_ansi_codes(s).chars().count()
    }
  }

  /// Helper method to render trees from root branches
  pub fn render<W: Write>(&mut self, writer: &mut W, roots: &[String], delimeter: Option<&str>) -> io::Result<()> {
    for (i, root) in roots.iter().enumerate() {
      if let Some(delim) = delimeter {
        if i > 0 {
          write!(writer, "{delim}")?; // Add delimiter between trees
        }
      }
      let is_last_root = i == roots.len() - 1;
      self.render_tree(writer, root, 0, &[], is_last_root)?;
    }
    Ok(())
  }

  /// Render the tree starting from a given branch
  pub fn render_tree<W: Write>(
    &mut self,
    writer: &mut W,
    branch_name: &str,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
  ) -> io::Result<()> {
    // Check max depth
    if let Some(max_depth) = self.max_depth {
      if depth > max_depth {
        return Ok(());
      }
    }

    // Check if we've already visited this branch
    if self.visited.contains(branch_name) {
      return Ok(());
    }

    // Mark as visited
    self.visited.insert(branch_name.to_string());

    // Get the branch node
    let node = match self.branch_nodes.get(branch_name) {
      Some(node) => node,
      None => return Ok(()),
    };

    // Print the branch with its prefix
    self.print_branch(writer, node, depth, prefix, is_last_sibling)?;

    // Prepare children for rendering
    let children = node.children.clone();
    let child_count = children.len();

    for (i, child) in children.iter().enumerate() {
      let is_last = i == child_count - 1;

      // Create a new prefix for the child
      let mut new_prefix: Vec<String> = prefix.to_vec();
      if depth > 0 {
        new_prefix.push(if is_last_sibling {
          "    ".to_string()
        } else {
          "│   ".to_string()
        });
      }

      // Render the child
      self.render_tree(writer, child, depth + 1, &new_prefix, is_last)?;
    }

    Ok(())
  }

  /// Print a branch with its metadata
  pub fn print_branch<W: Write>(
    &self,
    writer: &mut W,
    node: &BranchNode,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
  ) -> io::Result<()> {
    // Build the line prefix
    let mut line = String::new();

    // Add the prefix for all but the last level
    for p in prefix {
      line.push_str(p);
    }

    // Add the branch symbol
    if depth > 0 {
      let tree_symbol = if is_last_sibling { "└── " } else { "├── " };
      line.push_str(tree_symbol);
    }

    // Add the branch name
    let branch_display = if node.is_current {
      if self.no_color {
        format!("{} (current)", node.name)
      } else {
        format!("{} (current)", node.name.green().bold())
      }
    } else {
      node.name.clone()
    };
    line.push_str(&branch_display);

    // Calculate current display width (without ANSI codes)
    let current_width = self.display_width(&line);

    // Only add metadata if there's something to show
    let has_jira = node
      .metadata
      .as_ref()
      .and_then(|issue| issue.jira_issue.as_ref())
      .map(|jira| !jira.is_empty())
      .unwrap_or(false);
    let has_pr = node.metadata.as_ref().and_then(|issue| issue.github_pr).is_some();
    let has_cross_refs = self
      .cross_refs
      .get(&node.name)
      .map(|parents| parents.len() > 1 && parents.iter().any(|parent| !node.parents.contains(parent)))
      .unwrap_or(false);

    if has_jira || has_pr || has_cross_refs {
      // Use tree width for metadata alignment with proper spacing
      let jira_column_pos = std::cmp::max(current_width + 2, self.tree_width);
      let pr_column_pos = jira_column_pos + 12; // Space for "[JIRA-123]"
      let cross_ref_column_pos = pr_column_pos + 12; // Space for "[PR#123]"

      // Add issue/PR metadata with proper alignment
      if let Some(issue) = &node.metadata {
        let mut current_pos = current_width;

        // Add Jira issue if it exists and is not empty
        if has_jira {
          let spaces_needed = jira_column_pos.saturating_sub(current_pos);
          line.push_str(&" ".repeat(spaces_needed));

          let jira_issue = issue.jira_issue.as_ref().unwrap();
          let jira_display = if self.no_color {
            format!("[{jira_issue}]",)
          } else {
            format!("[{}]", jira_issue.cyan())
          };
          line.push_str(&jira_display);
          current_pos = self.display_width(&line);
        }

        // Add GitHub PR if available
        if let Some(pr_number) = issue.github_pr {
          // Always position PRs at the PR column position for consistent alignment
          let spaces_needed = pr_column_pos.saturating_sub(current_pos);
          line.push_str(&" ".repeat(spaces_needed));

          let pr_display = if self.no_color {
            format!("[PR#{pr_number}]")
          } else {
            format!("[PR#{}]", pr_number.to_string().yellow())
          };
          line.push_str(&pr_display);
        }
      }

      // Add cross-references with alignment (only if they exist)
      if let Some(parents) = self.cross_refs.get(&node.name) {
        if parents.len() > 1 {
          // Filter out parents that are already shown in the current branch path
          let other_parents: Vec<&String> = parents.iter().filter(|parent| !node.parents.contains(parent)).collect();

          if !other_parents.is_empty() {
            let current_width_final = self.display_width(&line);
            let spaces_needed = cross_ref_column_pos.saturating_sub(current_width_final);
            line.push_str(&" ".repeat(spaces_needed));

            let other_parents_str = other_parents
              .iter()
              .map(|s| s.as_str())
              .collect::<Vec<&str>>()
              .join(", ");
            let cross_ref_display = if self.no_color {
              format!("[also: {other_parents_str}]")
            } else {
              format!("[also: {}]", other_parents_str.dimmed())
            };
            line.push_str(&cross_ref_display);
          }
        }
      }
    }

    // Write the complete line to the writer
    writeln!(writer, "{line}")
  }

  /// Initialize the renderer with proper tree width calculation
  pub fn new(
    branch_nodes: &'a HashMap<String, BranchNode>,
    roots: &[String],
    max_depth: Option<u32>,
    no_color: bool,
  ) -> Self {
    let mut renderer = Self {
      branch_nodes,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth,
      no_color,
      tree_width: 0,
    };

    // Calculate tree width before rendering
    renderer.tree_width = renderer.calculate_max_tree_width(roots);
    renderer.build_cross_references();

    renderer
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use insta::assert_snapshot;

  use super::*;
  use crate::repo_state::BranchMetadata;

  #[test]
  fn test_build_cross_references_single_parent() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", true, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );

    let mut renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 0,
    };

    renderer.build_cross_references();
    assert!(renderer.cross_refs.is_empty());
  }

  #[test]
  fn test_build_cross_references_multiple_parents() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "develop".to_string(),
      create_test_branch("develop", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch(
        "feature1",
        true,
        vec!["main".to_string(), "develop".to_string()],
        vec![],
      ),
    );

    let mut renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 0,
    };

    renderer.build_cross_references();
    assert_eq!(renderer.cross_refs.len(), 1);
    assert!(renderer.cross_refs.contains_key("feature1"));

    let parents = &renderer.cross_refs["feature1"];
    assert_eq!(parents.len(), 2);
    assert!(parents.contains(&"main".to_string()));
    assert!(parents.contains(&"develop".to_string()));
  }

  #[test]
  fn test_calculate_max_tree_width_simple() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", true, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );

    let renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 0,
    };

    let roots = vec!["main".to_string()];
    let width = renderer.calculate_max_tree_width(&roots);

    // Should be at least the length of the longest branch path plus padding
    assert!(width > 0);
    assert!(width > "main (current)".len());
  }

  #[test]
  fn test_calculate_max_tree_width_with_depth() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch(
        "feature1",
        false,
        vec!["main".to_string()],
        vec!["subfeature".to_string()],
      ),
    );
    branches.insert(
      "subfeature".to_string(),
      create_test_branch("subfeature", true, vec!["feature1".to_string()], vec![]),
    );

    let renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: Some(1),
      no_color: true,
      tree_width: 0,
    };

    let roots = vec!["main".to_string()];
    let width_limited = renderer.calculate_max_tree_width(&roots);

    let renderer_unlimited = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 0,
    };

    let width_unlimited = renderer_unlimited.calculate_max_tree_width(&roots);

    // With depth limit, width should be different (typically smaller)
    assert!(width_limited > 0);
    assert!(width_unlimited > 0);
  }

  #[test]
  fn test_display_width_no_color() {
    let renderer = TreeRenderer {
      branch_nodes: &HashMap::new(),
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 0,
    };

    let text = "hello world";
    assert_eq!(renderer.display_width(text), 11);
  }

  #[test]
  fn test_render_tree_visits_branches() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", true, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );

    let mut renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 20,
    };

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    assert!(renderer.visited.contains("main"));
    assert!(renderer.visited.contains("feature1"));
  }

  #[test]
  fn test_render_tree_respects_max_depth() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch(
        "feature1",
        false,
        vec!["main".to_string()],
        vec!["subfeature".to_string()],
      ),
    );
    branches.insert(
      "subfeature".to_string(),
      create_test_branch("subfeature", false, vec!["feature1".to_string()], vec![]),
    );

    let mut renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: Some(1),
      no_color: true,
      tree_width: 20,
    };

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    assert!(renderer.visited.contains("main"));
    assert!(renderer.visited.contains("feature1"));
    // subfeature should not be visited due to depth limit
    assert!(!renderer.visited.contains("subfeature"));
  }

  #[test]
  fn test_render_tree_avoids_revisiting() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );

    let mut renderer = TreeRenderer {
      branch_nodes: &branches,
      visited: HashSet::new(),
      cross_refs: HashMap::new(),
      max_depth: None,
      no_color: true,
      tree_width: 20,
    };

    // Pre-mark a branch as visited
    renderer.visited.insert("feature1".to_string());

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    // main should be visited, but feature1 was already visited so should not be
    // processed again
    assert!(renderer.visited.contains("main"));
    assert!(renderer.visited.contains("feature1"));
    assert_eq!(renderer.visited.len(), 2);
  }

  #[test]
  fn test_render_tree_snapshot_basic() {
    let mut nodes = HashMap::new();

    // Create test nodes
    nodes.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature-1".to_string()]),
    );

    nodes.insert(
      "feature-1".to_string(),
      create_test_branch("feature-1", true, vec!["main".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert_snapshot!("basic_tree", output_str);
  }

  #[test]
  fn test_render_tree_with_jira_issues() {
    let mut nodes = HashMap::new();

    // Create test nodes
    nodes.insert(
      "main".to_string(),
      create_test_branch(
        "main",
        false,
        vec![],
        vec![
          "PROJ-123/feat-one".to_string(),
          "PROJ-124/feat-two-add-more-hats".to_string(),
        ],
      ),
    );
    nodes.insert(
      "PROJ-123/feat-one".to_string(),
      create_test_branch_with_metadata(
        "PROJ-123/feat-one",
        true,
        vec!["main".to_string()],
        vec![],
        Some("PROJ-123"),
        Some(456),
      ),
    );
    nodes.insert(
      "PROJ-124/feat-two-add-more-hats".to_string(),
      create_test_branch_with_metadata(
        "PROJ-124/feat-two-add-more-hats",
        true,
        vec!["main".to_string()],
        vec![],
        Some("PROJ-124"),
        Some(789),
      ),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert!(output_str.contains("PROJ-123/feat-one"));
    assert!(output_str.contains("PROJ-124/feat-two-add-more-hats"));
    assert!(output_str.contains("[PROJ-123]"));
    assert!(output_str.contains("[PROJ-124]"));
    assert!(output_str.contains("[PR#456]"));
    assert!(output_str.contains("[PR#789]"));
    assert_snapshot!("tree_with_jira_issue", output_str);
  }
  #[test]
  fn test_render_tree_with_diamond_case() {
    let mut nodes = HashMap::new();

    // Create diamond pattern:
    //       main
    //      /    \
    //   left    right
    //      \    /
    //      merge

    nodes.insert(
      "main".to_string(),
      create_test_branch(
        "main",
        false,
        vec![],
        vec!["PROJ-100/left-branch".to_string(), "PROJ-200/right-branch".to_string()],
      ),
    );

    nodes.insert(
      "PROJ-100/left-branch".to_string(),
      create_test_branch_with_metadata(
        "PROJ-100/left-branch",
        false,
        vec!["main".to_string()],
        vec!["PROJ-300/merge-branch".to_string()],
        Some("PROJ-100"),
        Some(111),
      ),
    );

    nodes.insert(
      "PROJ-200/right-branch".to_string(),
      create_test_branch_with_metadata(
        "PROJ-200/right-branch",
        false,
        vec!["main".to_string()],
        vec!["PROJ-300/merge-branch".to_string()],
        Some("PROJ-200"),
        Some(222),
      ),
    );

    nodes.insert(
      "PROJ-300/merge-branch".to_string(),
      create_test_branch_with_metadata(
        "PROJ-300/merge-branch",
        true,
        vec!["PROJ-100/left-branch".to_string(), "PROJ-200/right-branch".to_string()],
        vec![],
        Some("PROJ-300"),
        Some(333),
      ),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    // Verify all branches are present
    assert!(output_str.contains("PROJ-100/left-branch"));
    assert!(output_str.contains("PROJ-200/right-branch"));
    assert!(output_str.contains("PROJ-300/merge-branch"));

    // Verify JIRA issues are present
    assert!(output_str.contains("[PROJ-100]"));
    assert!(output_str.contains("[PROJ-200]"));
    assert!(output_str.contains("[PROJ-300]"));

    // Verify PR numbers are present
    assert!(output_str.contains("[PR#111]"));
    assert!(output_str.contains("[PR#222]"));
    assert!(output_str.contains("[PR#333]"));

    assert_snapshot!("tree_with_diamond", output_str);
  }
  #[test]
  fn test_new_initializes_correctly() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", true, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];
    let renderer = TreeRenderer::new(&branches, &roots, Some(5), false);

    assert_eq!(renderer.max_depth, Some(5));
    assert!(!renderer.no_color);
    assert!(renderer.tree_width > 0);
    assert!(renderer.visited.is_empty());
    assert!(renderer.cross_refs.is_empty()); // No multi-parent branches in this test
  }

  #[test]
  fn test_new_with_cross_references() {
    let mut branches = HashMap::new();
    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "develop".to_string(),
      create_test_branch("develop", false, vec![], vec!["feature1".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch(
        "feature1",
        true,
        vec!["main".to_string(), "develop".to_string()],
        vec![],
      ),
    );

    let roots = vec!["main".to_string(), "develop".to_string()];
    let renderer = TreeRenderer::new(&branches, &roots, None, true);

    assert!(renderer.no_color);
    assert!(!renderer.cross_refs.is_empty());
    assert!(renderer.cross_refs.contains_key("feature1"));
  }

  #[test]
  fn test_print_branch_github_pr_only_padding() {
    // Test that when branch_issue exists but jira_issue is None,
    // the GitHub PR is positioned correctly with proper padding
    let mut nodes = HashMap::new();

    // Create a branch with only GitHub PR (no JIRA issue)
    nodes.insert(
      "feature-branch".to_string(),
      BranchNode {
        name: "feature-branch".to_string(),
        is_current: false,
        metadata: Some(BranchMetadata {
          branch: "feature-branch".to_string(),
          jira_issue: None,     // No JIRA issue
          github_pr: Some(123), // Has GitHub PR
          created_at: "2023-01-01T00:00:00Z".to_string(),
        }),
        parents: vec![],
        children: vec![],
      },
    );

    let roots = vec!["feature-branch".to_string()];
    let renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Create a mock output buffer
    let mut output = Vec::new();

    // Test the print_branch method directly
    let node = &nodes["feature-branch"];
    renderer.print_branch(&mut output, node, 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    // The GitHub PR should be positioned at the correct column
    // When there's no JIRA issue, the PR should be padded to the JIRA column
    // position
    assert!(output_str.contains("#123"));

    // Should not contain any JIRA issue indicators
    assert!(!output_str.contains("PROJ-"));
    assert!(!output_str.contains("ABC-"));

    // Verify the PR appears in the expected format
    assert!(output_str.contains("[PR#123]"));
  }

  #[test]
  fn test_print_branch_both_jira_and_github() {
    // Test that when both JIRA issue and GitHub PR exist, they're positioned
    // correctly
    let mut nodes = HashMap::new();

    nodes.insert(
      "PROJ-123/feature-branch".to_string(),
      BranchNode {
        name: "PROJ-123/feature-branch".to_string(),
        is_current: false,
        metadata: Some(BranchMetadata {
          branch: "PROJ-123/feature-branch".to_string(),
          jira_issue: Some("PROJ-123".to_string()),
          github_pr: Some(456),
          created_at: "2023-01-01T00:00:00Z".to_string(),
        }),
        parents: vec![],
        children: vec![],
      },
    );

    let roots = vec!["PROJ-123/feature-branch".to_string()];
    let renderer = TreeRenderer::new(&nodes, &roots, None, true);

    let mut output = Vec::new();
    let node = &nodes["PROJ-123/feature-branch"];
    renderer.print_branch(&mut output, node, 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    // Should contain both JIRA issue and GitHub PR
    assert!(output_str.contains("PROJ-123"));
    assert!(output_str.contains("#456"));
    assert!(output_str.contains("[PROJ-123]"));
    assert!(output_str.contains("[PR#456]"));

    assert_snapshot!(output_str, @"PROJ-123/feature-branch  [PROJ-123]  [PR#456]");
  }

  #[test]
  fn test_print_branch_padding_alignment() {
    // Test that GitHub PR padding creates proper alignment between branches
    let mut nodes = HashMap::new();

    // Branch with JIRA issue and PR
    nodes.insert(
      "ABC-456/long-branch-name".to_string(),
      BranchNode {
        name: "ABC-456/long-branch-name".to_string(),
        is_current: false,
        metadata: Some(BranchMetadata {
          branch: "ABC-456/long-branch-name".to_string(),
          jira_issue: Some("ABC-456".to_string()),
          github_pr: Some(789),
          created_at: "2023-01-01T00:00:00Z".to_string(),
        }),
        parents: vec![],
        children: vec![],
      },
    );

    // Branch with only PR (should be padded to same column as JIRA)
    nodes.insert(
      "short".to_string(),
      BranchNode {
        name: "short".to_string(),
        is_current: false,
        metadata: Some(BranchMetadata {
          branch: "short".to_string(),
          jira_issue: None,
          github_pr: Some(321),
          created_at: "2023-01-01T00:00:00Z".to_string(),
        }),
        parents: vec![],
        children: vec![],
      },
    );

    let roots = vec!["ABC-456/long-branch-name".to_string(), "short".to_string()];
    let renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Test both branches
    let mut output1 = Vec::new();
    let node1 = &nodes["ABC-456/long-branch-name"];
    renderer.print_branch(&mut output1, node1, 0, &[], true).unwrap();
    let output1_str = String::from_utf8(output1).unwrap();

    let mut output2 = Vec::new();
    let node2 = &nodes["short"];
    renderer.print_branch(&mut output2, node2, 0, &[], true).unwrap();
    let output2_str = String::from_utf8(output2).unwrap();

    // Both should contain their respective PRs
    assert!(output1_str.contains("[PR#789]"));
    assert!(output2_str.contains("[PR#321]"));

    // The short branch's PR should be padded to align with the JIRA column
    // This means there should be significant spacing before [PR#321]
    let pr_position = output2_str.find("[PR#321]").unwrap();
    assert!(pr_position > "short".len() + 5); // At least some padding
  }

  #[test]
  fn test_render_tree_with_multiple_roots() {
    let mut nodes = HashMap::new();

    // Create test nodes with two root branches
    nodes.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature-a".to_string()]),
    );

    nodes.insert(
      "develop".to_string(),
      create_test_branch("develop", false, vec![], vec!["feature-b".to_string()]),
    );

    nodes.insert(
      "feature-a".to_string(),
      create_test_branch("feature-a", false, vec!["main".to_string()], vec![]),
    );

    nodes.insert(
      "feature-b".to_string(),
      create_test_branch("feature-b", true, vec!["develop".to_string()], vec![]),
    );

    let roots = vec!["main".to_string(), "develop".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render(&mut output, &roots, Some("\n")).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert_snapshot!("tree_with_multiple_roots", output_str);
  }

  #[test]
  fn test_render_tree_with_max_depth() {
    let mut nodes = HashMap::new();

    // Create a deep tree structure
    nodes.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["level1".to_string()]),
    );
    nodes.insert(
      "level1".to_string(),
      create_test_branch("level1", false, vec!["main".to_string()], vec!["level2".to_string()]),
    );
    nodes.insert(
      "level2".to_string(),
      create_test_branch("level2", false, vec!["level1".to_string()], vec!["level3".to_string()]),
    );
    nodes.insert(
      "level3".to_string(),
      create_test_branch("level3", true, vec!["level2".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];

    // Create renderer with max_depth=2
    let mut renderer = TreeRenderer::new(&nodes, &roots, Some(2), true);

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert_snapshot!("tree_with_max_depth", output_str);
  }

  #[test]
  fn test_render_tree_with_cross_references() {
    let mut nodes = HashMap::new();

    // Create a structure where a branch has multiple parents
    nodes.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["shared-feature".to_string()]),
    );
    nodes.insert(
      "develop".to_string(),
      create_test_branch("develop", false, vec![], vec!["shared-feature".to_string()]),
    );
    nodes.insert(
      "release".to_string(),
      create_test_branch("release", false, vec![], vec!["shared-feature".to_string()]),
    );
    nodes.insert(
      "shared-feature".to_string(),
      create_test_branch(
        "shared-feature",
        true,
        vec!["main".to_string(), "develop".to_string(), "release".to_string()],
        vec![],
      ),
    );

    let roots = vec!["main".to_string(), "develop".to_string(), "release".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render(&mut output, &roots, Some("\n")).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert_snapshot!("tree_with_cross_references", output_str);
  }

  #[test]
  fn test_render_tree_with_github_pr_only() {
    let mut nodes = HashMap::new();

    // Create a branch with only GitHub PR (no JIRA issue)
    nodes.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature-pr-only".to_string()]),
    );

    nodes.insert(
      "feature-pr-only".to_string(),
      BranchNode {
        name: "feature-pr-only".to_string(),
        is_current: true,
        metadata: Some(BranchMetadata {
          branch: "feature-pr-only".to_string(),
          jira_issue: None,     // No JIRA issue
          github_pr: Some(123), // Has GitHub PR
          created_at: "2023-01-01T00:00:00Z".to_string(),
        }),
        parents: vec!["main".to_string()],
        children: vec![],
      },
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert_snapshot!("tree_with_github_pr_only", output_str);
  }

  #[test]
  fn test_render_tree_with_deep_nesting() {
    let mut nodes = HashMap::new();

    // Create a deeply nested tree structure (5 levels)
    nodes.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string()]),
    );
    nodes.insert(
      "feature1".to_string(),
      create_test_branch_with_metadata(
        "feature1",
        false,
        vec!["main".to_string()],
        vec!["feature1-1".to_string()],
        Some("FEAT-1"),
        Some(1),
      ),
    );
    nodes.insert(
      "feature1-1".to_string(),
      create_test_branch_with_metadata(
        "feature1-1",
        false,
        vec!["feature1".to_string()],
        vec!["feature1-1-1".to_string()],
        Some("FEAT-2"),
        None,
      ),
    );
    nodes.insert(
      "feature1-1-1".to_string(),
      create_test_branch_with_metadata(
        "feature1-1-1",
        false,
        vec!["feature1-1".to_string()],
        vec!["feature1-1-1-1".to_string()],
        None,
        Some(41),
      ),
    );
    nodes.insert(
      "feature1-1-1-1".to_string(),
      create_test_branch_with_metadata(
        "feature1-1-1-1",
        true,
        vec!["feature1-1-1".to_string()],
        vec![],
        Some("FEAT-4"),
        Some(3),
      ),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&nodes, &roots, None, true);

    // Render the tree to a buffer
    let mut output = Vec::new();
    renderer.render_tree(&mut output, "main", 0, &[], true).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    assert_snapshot!("tree_with_deep_nesting", output_str);
  }

  fn create_test_branch(name: &str, is_current: bool, parents: Vec<String>, children: Vec<String>) -> BranchNode {
    BranchNode {
      name: name.to_string(),
      is_current,
      metadata: None,
      parents,
      children,
    }
  }

  fn create_test_branch_with_metadata(
    name: &str,
    is_current: bool,
    parents: Vec<String>,
    children: Vec<String>,
    jira_issue: Option<&str>,
    github_pr: Option<u32>,
  ) -> BranchNode {
    BranchNode {
      name: name.to_string(),
      is_current,
      metadata: Some(BranchMetadata {
        branch: name.to_string(),
        jira_issue: match jira_issue {
          Some(s) => Some(s.to_string()),
          None => None,
        },
        github_pr,
        created_at: "".to_string(),
      }),
      parents,
      children,
    }
  }
}
