//! # Branch Tree Renderer
//!
//! Provides tree visualization and rendering functionality for Git branch
//! dependencies, including formatting, coloring, and hierarchical display.

use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

use owo_colors::OwoColorize;

use crate::diamond_detector::{DiamondDetector, DiamondPattern};
use crate::state::BranchMetadata;

/// Information about a branch's role in diamond patterns
#[derive(Debug, Default)]
pub struct DiamondInfo {
  pub is_diamond_ancestor: bool,
  pub is_diamond_merge: bool,
  pub is_diamond_path: bool,
  pub diamond_roles: Vec<String>,
}

/// Information about a branch's cross-reference status
#[derive(Debug, Default)]
pub struct CrossRefInfo {
  pub has_multiple_parents: bool,
  pub is_cross_referenced: bool,
  pub is_in_circular_dep: bool,
  pub reference_count: usize,
}

/// Configuration for deep nesting rendering
#[derive(Debug, Clone)]
pub struct DeepNestingConfig {
  pub max_depth: Option<u32>,
  pub max_branches_per_level: Option<usize>,
  pub enable_pagination: bool,
  pub page_size: usize,
  pub show_depth_indicators: bool,
  pub enable_pruning: bool,
  pub prune_threshold: usize,
}

impl Default for DeepNestingConfig {
  fn default() -> Self {
    Self {
      max_depth: Some(20),
      max_branches_per_level: Some(50),
      enable_pagination: true,
      page_size: 10,
      show_depth_indicators: true,
      enable_pruning: true,
      prune_threshold: 100,
    }
  }
}

/// Statistics about tree rendering
#[derive(Debug, Default)]
pub struct RenderStats {
  pub total_branches: usize,
  pub max_depth_reached: u32,
  pub branches_pruned: usize,
  pub circular_deps_detected: usize,
  pub memory_usage_estimate: usize,
}

/// Commit ahead/behind information for a branch relative to its parent.
#[derive(Debug, Clone)]
pub struct CommitInfo {
  /// Number of commits the branch is ahead of its parent.
  pub ahead: usize,
  /// Number of commits the branch is behind its parent.
  pub behind: usize,
}

/// Represents a branch node in the tree
#[derive(Debug, Clone)]
pub struct BranchNode {
  pub name: String,
  pub is_current: bool,
  pub metadata: Option<BranchMetadata>,
  pub parents: Vec<String>,
  pub children: Vec<String>,
  /// Commit ahead/behind counts relative to the branch's parent.
  pub commit_info: Option<CommitInfo>,
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
    if let Some(max_depth) = self.max_depth
      && depth > max_depth
    {
      return 0;
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
      if let Some(delim) = delimeter
        && i > 0
      {
        write!(writer, "{delim}")?; // Add delimiter between trees
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
    if let Some(max_depth) = self.max_depth
      && depth > max_depth
    {
      return Ok(());
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
    let has_commit_info = node.commit_info.is_some();

    if has_jira || has_pr || has_cross_refs || has_commit_info {
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
      if let Some(parents) = self.cross_refs.get(&node.name)
        && parents.len() > 1
      {
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

      // Add ahead/behind commit counts
      if let Some(info) = &node.commit_info {
        if info.ahead > 0 || info.behind > 0 {
          let current_width_now = self.display_width(&line);
          let spaces_needed = if current_width_now > current_width {
            2 // At least 2 spaces padding after previous badges
          } else {
            std::cmp::max(current_width + 2, self.tree_width).saturating_sub(current_width_now)
          };
          line.push_str(&" ".repeat(spaces_needed));

          let mut parts = Vec::new();
          if info.ahead > 0 {
            parts.push(format!("↑{}", info.ahead));
          }
          if info.behind > 0 {
            parts.push(format!("↓{}", info.behind));
          }
          let counts = parts.join(" ");
          let display = if self.no_color {
            format!("[{counts}]")
          } else {
            format!("[{}]", counts.dimmed())
          };
          line.push_str(&display);
        }
      }
    }

    // Write the complete line to the writer
    writeln!(writer, "{line}")
  }

  /// Add branch metadata (Jira, PR, cross-refs, ahead/behind) to a line
  fn add_branch_metadata(&self, line: &mut String, node: &BranchNode) {
    let current_width = self.display_width(line);

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
    let has_commit_info = node.commit_info.is_some();

    if !has_jira && !has_pr && !has_cross_refs && !has_commit_info {
      return;
    }

    let jira_column_pos = std::cmp::max(current_width + 2, self.tree_width);
    let pr_column_pos = jira_column_pos + 12;
    let cross_ref_column_pos = pr_column_pos + 12;

    if let Some(issue) = &node.metadata {
      let mut current_pos = current_width;

      if has_jira {
        let spaces_needed = jira_column_pos.saturating_sub(current_pos);
        line.push_str(&" ".repeat(spaces_needed));

        let jira_issue = issue.jira_issue.as_ref().unwrap();
        let jira_display = if self.no_color {
          format!("[{jira_issue}]")
        } else {
          format!("[{}]", jira_issue.cyan())
        };
        line.push_str(&jira_display);
        current_pos = self.display_width(line);
      }

      if let Some(pr_number) = issue.github_pr {
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

    if let Some(parents) = self.cross_refs.get(&node.name)
      && parents.len() > 1
    {
      let other_parents: Vec<&String> = parents.iter().filter(|parent| !node.parents.contains(parent)).collect();

      if !other_parents.is_empty() {
        let current_width_final = self.display_width(line);
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

    if let Some(info) = &node.commit_info {
      if info.ahead > 0 || info.behind > 0 {
        let current_width_now = self.display_width(line);
        let spaces_needed = if current_width_now > current_width {
          2
        } else {
          std::cmp::max(current_width + 2, self.tree_width).saturating_sub(current_width_now)
        };
        line.push_str(&" ".repeat(spaces_needed));

        let mut parts = Vec::new();
        if info.ahead > 0 {
          parts.push(format!("↑{}", info.ahead));
        }
        if info.behind > 0 {
          parts.push(format!("↓{}", info.behind));
        }
        let counts = parts.join(" ");
        let display = if self.no_color {
          format!("[{counts}]")
        } else {
          format!("[{}]", counts.dimmed())
        };
        line.push_str(&display);
      }
    }
  }

  // ─── Circular dependency detection ────────────────────────────────────

  /// Detect circular dependencies in the branch graph
  pub fn detect_circular_dependencies(&self) -> Vec<Vec<String>> {
    let mut circular_deps = Vec::new();
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for (branch_name, _) in self.branch_nodes {
      if !visited.contains(branch_name) {
        if let Some(cycle) =
          self.find_cycle_from_branch(branch_name, &mut visited, &mut rec_stack, &mut Vec::new())
        {
          circular_deps.push(cycle);
        }
      }
    }

    circular_deps
  }

  /// Find cycles starting from a specific branch using DFS
  fn find_cycle_from_branch(
    &self,
    branch: &str,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
  ) -> Option<Vec<String>> {
    visited.insert(branch.to_string());
    rec_stack.insert(branch.to_string());
    path.push(branch.to_string());

    if let Some(node) = self.branch_nodes.get(branch) {
      for child in &node.children {
        if !visited.contains(child) {
          if let Some(cycle) = self.find_cycle_from_branch(child, visited, rec_stack, path) {
            return Some(cycle);
          }
        } else if rec_stack.contains(child) {
          // Found a cycle - extract the cycle from the path
          let cycle_start = path.iter().position(|x| x == child).unwrap_or(0);
          let mut cycle = path[cycle_start..].to_vec();
          cycle.push(child.to_string()); // Close the cycle
          return Some(cycle);
        }
      }
    }

    path.pop();
    rec_stack.remove(branch);
    None
  }

  // ─── Enhanced cross-reference rendering ───────────────────────────────

  /// Enhanced cross-reference rendering with better visual indicators
  pub fn render_with_enhanced_cross_refs<W: Write>(
    &mut self,
    writer: &mut W,
    roots: &[String],
    delimiter: Option<&str>,
    show_cross_refs: bool,
    max_ref_depth: Option<u32>,
  ) -> io::Result<()> {
    let circular_deps = if show_cross_refs {
      self.detect_circular_dependencies()
    } else {
      Vec::new()
    };

    if !circular_deps.is_empty() {
      writeln!(writer, "⚠️  Circular dependencies detected:")?;
      for (i, cycle) in circular_deps.iter().enumerate() {
        writeln!(writer, "  Cycle {}: {}", i + 1, cycle.join(" → "))?;
      }
      writeln!(writer)?;
    }

    self.visited.clear();

    for (i, root) in roots.iter().enumerate() {
      if let Some(delim) = delimiter
        && i > 0
      {
        write!(writer, "{delim}")?;
      }
      let is_last_root = i == roots.len() - 1;
      self.render_tree_with_enhanced_cross_refs(
        writer,
        root,
        0,
        &[],
        is_last_root,
        max_ref_depth,
        &circular_deps,
      )?;
    }

    if show_cross_refs && !self.cross_refs.is_empty() {
      writeln!(writer)?;
      writeln!(writer, "📎 Cross-references summary:")?;
      for (branch, parents) in &self.cross_refs {
        writeln!(writer, "  {} ← {}", branch, parents.join(", "))?;
      }
    }

    Ok(())
  }

  /// Render tree with enhanced cross-reference handling
  fn render_tree_with_enhanced_cross_refs<W: Write>(
    &mut self,
    writer: &mut W,
    branch_name: &str,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    max_ref_depth: Option<u32>,
    circular_deps: &[Vec<String>],
  ) -> io::Result<()> {
    if let Some(max_depth) = self.max_depth
      && depth > max_depth
    {
      return Ok(());
    }

    let is_in_circular_dep = circular_deps
      .iter()
      .any(|cycle| cycle.contains(&branch_name.to_string()));

    if self.visited.contains(branch_name) {
      self.print_branch_reference(writer, branch_name, depth, prefix, is_last_sibling, is_in_circular_dep)?;
      return Ok(());
    }

    self.visited.insert(branch_name.to_string());

    let node = match self.branch_nodes.get(branch_name) {
      Some(node) => node,
      None => return Ok(()),
    };

    let cross_ref_info = CrossRefInfo {
      has_multiple_parents: node.parents.len() > 1,
      is_cross_referenced: self.cross_refs.contains_key(branch_name),
      is_in_circular_dep,
      reference_count: self.count_branch_references(branch_name),
    };

    self.print_branch_with_cross_refs(writer, node, depth, prefix, is_last_sibling, &cross_ref_info)?;

    let children = node.children.clone();
    let child_count = children.len();

    for (i, child) in children.iter().enumerate() {
      let is_last = i == child_count - 1;

      let should_render_child = if let Some(max_ref_depth) = max_ref_depth {
        depth < max_ref_depth || !self.cross_refs.contains_key(child)
      } else {
        true
      };

      if should_render_child {
        let mut new_prefix = prefix.to_vec();
        if depth > 0 {
          new_prefix.push(if is_last { "    ".to_string() } else { "│   ".to_string() });
        }

        self.render_tree_with_enhanced_cross_refs(
          writer,
          child,
          depth + 1,
          &new_prefix,
          is_last,
          max_ref_depth,
          circular_deps,
        )?;
      }
    }

    Ok(())
  }

  /// Print branch reference (when branch is revisited)
  fn print_branch_reference<W: Write>(
    &self,
    writer: &mut W,
    branch_name: &str,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    is_in_circular_dep: bool,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    if depth > 0 {
      let ref_symbol = if is_in_circular_dep {
        if is_last_sibling {
          "└🔄─ "
        } else {
          "├🔄─ "
        }
      } else if is_last_sibling {
        "└→─ "
      } else {
        "├→─ "
      };
      line.push_str(ref_symbol);
    }

    let display_name = if self.no_color {
      format!("{} (see above)", branch_name)
    } else {
      format!("{} {}", branch_name.dimmed(), "(see above)".italic().dimmed())
    };
    line.push_str(&display_name);

    if is_in_circular_dep {
      if self.no_color {
        line.push_str(" [CIRCULAR]");
      } else {
        let warning = " [CIRCULAR]".red().bold().to_string();
        line.push_str(&warning);
      }
    }

    writeln!(writer, "{line}")?;
    Ok(())
  }

  /// Count how many times a branch is referenced in the tree
  pub fn count_branch_references(&self, branch_name: &str) -> usize {
    let mut count = 0;
    for (_, node) in self.branch_nodes {
      if node.children.contains(&branch_name.to_string()) {
        count += 1;
      }
    }
    count
  }

  /// Print branch with enhanced cross-reference information
  fn print_branch_with_cross_refs<W: Write>(
    &self,
    writer: &mut W,
    node: &BranchNode,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    cross_ref_info: &CrossRefInfo,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    if depth > 0 {
      let tree_symbol = if cross_ref_info.is_in_circular_dep {
        if is_last_sibling {
          "└🔄─ "
        } else {
          "├🔄─ "
        }
      } else if cross_ref_info.has_multiple_parents {
        if is_last_sibling {
          "└◈─ "
        } else {
          "├◈─ "
        }
      } else if cross_ref_info.reference_count > 1 {
        if is_last_sibling {
          "└◇─ "
        } else {
          "├◇─ "
        }
      } else if is_last_sibling {
        "└── "
      } else {
        "├── "
      };
      line.push_str(tree_symbol);
    }

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

    if cross_ref_info.reference_count > 1 {
      let ref_indicator = if self.no_color {
        format!(" [refs:{}]", cross_ref_info.reference_count)
      } else {
        format!(" [refs:{}]", cross_ref_info.reference_count.to_string().blue())
      };
      line.push_str(&ref_indicator);
    }

    if cross_ref_info.is_in_circular_dep {
      if self.no_color {
        line.push_str(" [CIRCULAR]");
      } else {
        let circular_indicator = " [CIRCULAR]".red().bold().to_string();
        line.push_str(&circular_indicator);
      }
    }

    self.add_branch_metadata(&mut line, node);

    writeln!(writer, "{line}")?;
    Ok(())
  }

  // ─── Diamond pattern rendering ────────────────────────────────────────

  /// Render tree with enhanced diamond pattern visualization
  pub fn render_with_diamonds<W: Write>(
    &mut self,
    writer: &mut W,
    roots: &[String],
    delimiter: Option<&str>,
    show_diamonds: bool,
  ) -> io::Result<()> {
    let diamond_patterns = if show_diamonds {
      let detector = DiamondDetector::new(self.branch_nodes);
      detector.detect_diamond_patterns()
    } else {
      Vec::new()
    };

    self.visited.clear();

    for (i, root) in roots.iter().enumerate() {
      if let Some(delim) = delimiter
        && i > 0
      {
        write!(writer, "{delim}")?;
      }
      let is_last_root = i == roots.len() - 1;
      self.render_tree_with_diamonds(writer, root, 0, &[], is_last_root, &diamond_patterns)?;
    }
    Ok(())
  }

  /// Render tree branch with diamond pattern annotations
  pub fn render_tree_with_diamonds<W: Write>(
    &mut self,
    writer: &mut W,
    branch_name: &str,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    diamond_patterns: &[DiamondPattern],
  ) -> io::Result<()> {
    if let Some(max_depth) = self.max_depth
      && depth > max_depth
    {
      return Ok(());
    }

    if self.visited.contains(branch_name) {
      return Ok(());
    }

    self.visited.insert(branch_name.to_string());

    let node = match self.branch_nodes.get(branch_name) {
      Some(node) => node,
      None => return Ok(()),
    };

    let diamond_info = self.get_diamond_info(branch_name, diamond_patterns);

    self.print_branch_with_diamonds(writer, node, depth, prefix, is_last_sibling, &diamond_info)?;

    let children = node.children.clone();
    let child_count = children.len();

    for (i, child) in children.iter().enumerate() {
      let is_last = i == child_count - 1;

      let mut new_prefix = prefix.to_vec();
      if depth > 0 {
        new_prefix.push(if is_last { "    ".to_string() } else { "│   ".to_string() });
      }

      self.render_tree_with_diamonds(writer, child, depth + 1, &new_prefix, is_last, diamond_patterns)?;
    }

    Ok(())
  }

  /// Get diamond pattern information for a branch
  fn get_diamond_info(&self, branch_name: &str, diamond_patterns: &[DiamondPattern]) -> DiamondInfo {
    let mut info = DiamondInfo::default();

    for diamond in diamond_patterns {
      if branch_name == &diamond.ancestor {
        info.is_diamond_ancestor = true;
        info.diamond_roles.push("ancestor".to_string());
      }
      if branch_name == &diamond.merge_point {
        info.is_diamond_merge = true;
        info.diamond_roles.push("merge".to_string());
      }
      if diamond.left_path.contains(&branch_name.to_string()) {
        info.is_diamond_path = true;
        info.diamond_roles.push("left-path".to_string());
      }
      if diamond.right_path.contains(&branch_name.to_string()) {
        info.is_diamond_path = true;
        info.diamond_roles.push("right-path".to_string());
      }
    }

    info
  }

  /// Print branch with diamond pattern annotations
  pub fn print_branch_with_diamonds<W: Write>(
    &self,
    writer: &mut W,
    node: &BranchNode,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    diamond_info: &DiamondInfo,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    if depth > 0 {
      let tree_symbol = self.get_enhanced_tree_symbol(is_last_sibling, diamond_info);
      line.push_str(&tree_symbol);
    }

    let branch_display = self.format_branch_name_with_diamonds(node, diamond_info);
    line.push_str(&branch_display);

    self.add_branch_metadata(&mut line, node);

    writeln!(writer, "{line}")?;
    Ok(())
  }

  /// Get enhanced tree symbols for diamond patterns
  fn get_enhanced_tree_symbol(&self, is_last_sibling: bool, diamond_info: &DiamondInfo) -> String {
    if diamond_info.is_diamond_ancestor {
      if is_last_sibling { "└◇─ " } else { "├◇─ " }.to_string()
    } else if diamond_info.is_diamond_merge {
      if is_last_sibling { "└◅─ " } else { "├◅─ " }.to_string()
    } else if diamond_info.is_diamond_path {
      if is_last_sibling { "└◈─ " } else { "├◈─ " }.to_string()
    } else {
      if is_last_sibling { "└── " } else { "├── " }.to_string()
    }
  }

  /// Format branch name with diamond annotations
  fn format_branch_name_with_diamonds(&self, node: &BranchNode, diamond_info: &DiamondInfo) -> String {
    let base_name = if node.is_current {
      if self.no_color {
        format!("{} (current)", node.name)
      } else {
        format!("{} (current)", node.name.green().bold())
      }
    } else {
      node.name.clone()
    };

    if diamond_info.diamond_roles.is_empty() {
      return base_name;
    }

    let roles = diamond_info.diamond_roles.join(",");
    if self.no_color {
      format!("{} [{}]", base_name, roles)
    } else {
      format!("{} [{}]", base_name, roles.magenta())
    }
  }

  // ─── Deep nesting rendering ───────────────────────────────────────────

  /// Render with deep nesting support, pagination, and memory optimization
  pub fn render_with_deep_nesting<W: Write>(
    &mut self,
    writer: &mut W,
    roots: &[String],
    config: &DeepNestingConfig,
  ) -> io::Result<RenderStats> {
    let mut stats = RenderStats::default();

    stats.total_branches = self.branch_nodes.len();
    stats.memory_usage_estimate = self.estimate_memory_usage();

    let circular_deps = self.detect_circular_dependencies();
    stats.circular_deps_detected = circular_deps.len();

    if !circular_deps.is_empty() {
      writeln!(writer, "⚠️  {} circular dependencies detected", circular_deps.len())?;
      for (i, cycle) in circular_deps.iter().enumerate().take(3) {
        writeln!(writer, "  Cycle {}: {}", i + 1, cycle.join(" → "))?;
      }
      if circular_deps.len() > 3 {
        writeln!(writer, "  ... and {} more", circular_deps.len() - 3)?;
      }
      writeln!(writer)?;
    }

    if config.enable_pruning && stats.total_branches > config.prune_threshold {
      writeln!(
        writer,
        "🌳 Large tree detected ({} branches). Applying intelligent pruning...",
        stats.total_branches
      )?;
      writeln!(writer)?;
    }

    self.visited.clear();

    for (i, root) in roots.iter().enumerate() {
      if i > 0 {
        writeln!(writer)?;
      }

      let branch_stats = self.render_branch_with_deep_nesting(
        writer,
        root,
        0,
        &[],
        true,
        config,
        &mut stats,
        &circular_deps,
      )?;

      stats.max_depth_reached = stats.max_depth_reached.max(branch_stats.max_depth_reached);
      stats.branches_pruned += branch_stats.branches_pruned;
    }

    if config.show_depth_indicators {
      writeln!(writer)?;
      writeln!(writer, "📊 Rendering Statistics:")?;
      writeln!(writer, "  Total branches: {}", stats.total_branches)?;
      writeln!(writer, "  Max depth reached: {}", stats.max_depth_reached)?;
      if stats.branches_pruned > 0 {
        writeln!(writer, "  Branches pruned: {}", stats.branches_pruned)?;
      }
      if stats.circular_deps_detected > 0 {
        writeln!(writer, "  Circular dependencies: {}", stats.circular_deps_detected)?;
      }
      writeln!(writer, "  Memory estimate: {} bytes", stats.memory_usage_estimate)?;
    }

    Ok(stats)
  }

  /// Render a single branch with deep nesting support
  fn render_branch_with_deep_nesting<W: Write>(
    &mut self,
    writer: &mut W,
    branch_name: &str,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    config: &DeepNestingConfig,
    stats: &mut RenderStats,
    circular_deps: &[Vec<String>],
  ) -> io::Result<RenderStats> {
    let mut branch_stats = RenderStats::default();
    branch_stats.max_depth_reached = depth;

    if let Some(max_depth) = config.max_depth {
      if depth > max_depth {
        self.print_depth_truncation_indicator(writer, depth, prefix, is_last_sibling, max_depth)?;
        branch_stats.branches_pruned += 1;
        return Ok(branch_stats);
      }
    }

    let is_in_circular_dep = circular_deps
      .iter()
      .any(|cycle| cycle.contains(&branch_name.to_string()));

    if self.visited.contains(branch_name) {
      self.print_branch_reference(writer, branch_name, depth, prefix, is_last_sibling, is_in_circular_dep)?;
      return Ok(branch_stats);
    }

    self.visited.insert(branch_name.to_string());

    let node = match self.branch_nodes.get(branch_name) {
      Some(node) => node,
      None => return Ok(branch_stats),
    };

    if config.enable_pruning && self.should_prune_subtree(node, depth, config) {
      self.print_pruning_indicator(writer, branch_name, depth, prefix, is_last_sibling, node.children.len())?;
      branch_stats.branches_pruned += self.count_subtree_size(node);
      return Ok(branch_stats);
    }

    self.print_branch_with_depth_info(writer, node, depth, prefix, is_last_sibling, config, is_in_circular_dep)?;

    let children = node.children.clone();
    let child_count = children.len();

    if config.enable_pagination && child_count > config.page_size {
      for page in 0..((child_count + config.page_size - 1) / config.page_size) {
        let start = page * config.page_size;
        let end = std::cmp::min(start + config.page_size, child_count);

        if page > 0 {
          self.print_pagination_separator(writer, depth + 1, prefix, page, start, end, child_count)?;
        }

        for (i, child) in children[start..end].iter().enumerate() {
          let global_index = start + i;
          let is_last_in_page = i == (end - start - 1);
          let is_last_overall = global_index == child_count - 1;
          let is_last = is_last_in_page && is_last_overall;

          let mut new_prefix = prefix.to_vec();
          if depth > 0 {
            new_prefix.push(if is_last { "    ".to_string() } else { "│   ".to_string() });
          }

          let child_stats = self.render_branch_with_deep_nesting(
            writer,
            child,
            depth + 1,
            &new_prefix,
            is_last,
            config,
            stats,
            circular_deps,
          )?;

          branch_stats.max_depth_reached = branch_stats.max_depth_reached.max(child_stats.max_depth_reached);
          branch_stats.branches_pruned += child_stats.branches_pruned;
        }
      }
    } else {
      for (i, child) in children.iter().enumerate() {
        let is_last = i == child_count - 1;

        let mut new_prefix = prefix.to_vec();
        if depth > 0 {
          new_prefix.push(if is_last { "    ".to_string() } else { "│   ".to_string() });
        }

        let child_stats = self.render_branch_with_deep_nesting(
          writer,
          child,
          depth + 1,
          &new_prefix,
          is_last,
          config,
          stats,
          circular_deps,
        )?;

        branch_stats.max_depth_reached = branch_stats.max_depth_reached.max(child_stats.max_depth_reached);
        branch_stats.branches_pruned += child_stats.branches_pruned;
      }
    }

    Ok(branch_stats)
  }

  /// Print branch with depth information
  fn print_branch_with_depth_info<W: Write>(
    &self,
    writer: &mut W,
    node: &BranchNode,
    depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    config: &DeepNestingConfig,
    is_in_circular_dep: bool,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    if depth > 0 {
      let tree_symbol = if is_in_circular_dep {
        if is_last_sibling {
          "└🔄─ "
        } else {
          "├🔄─ "
        }
      } else if depth > 10 {
        if is_last_sibling {
          "└⼇─ "
        } else {
          "├⼇─ "
        }
      } else if is_last_sibling {
        "└── "
      } else {
        "├── "
      };
      line.push_str(tree_symbol);
    }

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

    if config.show_depth_indicators && depth > 0 {
      let depth_indicator = if self.no_color {
        format!(" [depth:{}]", depth)
      } else {
        format!(" [depth:{}]", depth.to_string().dimmed())
      };
      line.push_str(&depth_indicator);
    }

    if depth > 5 && !node.children.is_empty() {
      let child_indicator = if self.no_color {
        format!(" [children:{}]", node.children.len())
      } else {
        format!(" [children:{}]", node.children.len().to_string().blue())
      };
      line.push_str(&child_indicator);
    }

    if is_in_circular_dep {
      if self.no_color {
        line.push_str(" [CIRCULAR]");
      } else {
        let warning = " [CIRCULAR]".red().bold().to_string();
        line.push_str(&warning);
      }
    }

    self.add_branch_metadata(&mut line, node);

    writeln!(writer, "{line}")?;
    Ok(())
  }

  /// Print depth truncation indicator
  fn print_depth_truncation_indicator<W: Write>(
    &self,
    writer: &mut W,
    _depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    max_depth: u32,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    let symbol = if is_last_sibling { "└── " } else { "├── " };
    line.push_str(symbol);

    let truncation_msg = if self.no_color {
      format!("... [truncated at depth {}] ...", max_depth)
    } else {
      format!("... [truncated at depth {}] ...", max_depth.to_string().yellow())
    };
    line.push_str(&truncation_msg);

    writeln!(writer, "{line}")?;
    Ok(())
  }

  /// Print pruning indicator
  fn print_pruning_indicator<W: Write>(
    &self,
    writer: &mut W,
    branch_name: &str,
    _depth: u32,
    prefix: &[String],
    is_last_sibling: bool,
    child_count: usize,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    let symbol = if is_last_sibling { "└── " } else { "├── " };
    line.push_str(symbol);

    let pruning_msg = if self.no_color {
      format!(
        "{} ... [pruned subtree with {} children] ...",
        branch_name, child_count
      )
    } else {
      format!(
        "{} ... [pruned subtree with {} children] ...",
        branch_name,
        child_count.to_string().yellow()
      )
    };
    line.push_str(&pruning_msg);

    writeln!(writer, "{line}")?;
    Ok(())
  }

  /// Print pagination separator
  fn print_pagination_separator<W: Write>(
    &self,
    writer: &mut W,
    _depth: u32,
    prefix: &[String],
    page: usize,
    start: usize,
    end: usize,
    total: usize,
  ) -> io::Result<()> {
    let mut line = String::new();

    for p in prefix {
      line.push_str(p);
    }

    let separator_msg = if self.no_color {
      format!(
        "├── ... Page {} ({}-{} of {}) ...",
        page + 1,
        start + 1,
        end,
        total
      )
    } else {
      format!(
        "├── ... Page {} ({}-{} of {}) ...",
        (page + 1).to_string().cyan(),
        (start + 1).to_string().cyan(),
        end.to_string().cyan(),
        total.to_string().cyan()
      )
    };
    line.push_str(&separator_msg);

    writeln!(writer, "{line}")?;
    Ok(())
  }

  /// Check if a subtree should be pruned
  fn should_prune_subtree(&self, node: &BranchNode, depth: u32, config: &DeepNestingConfig) -> bool {
    if depth > 15 && node.children.len() > 10 {
      return true;
    }

    if let Some(max_branches) = config.max_branches_per_level {
      if node.children.len() > max_branches {
        return true;
      }
    }

    false
  }

  /// Count the size of a subtree (for pruning statistics)
  fn count_subtree_size(&self, node: &BranchNode) -> usize {
    let mut count = 1;
    let mut visited = HashSet::new();
    let mut to_visit = node.children.clone();

    while let Some(child_name) = to_visit.pop() {
      if visited.contains(&child_name) {
        continue;
      }
      visited.insert(child_name.clone());

      if let Some(child_node) = self.branch_nodes.get(&child_name) {
        count += 1;
        for grandchild in &child_node.children {
          if !visited.contains(grandchild) {
            to_visit.push(grandchild.clone());
          }
        }
      }

      if count > 1000 {
        break;
      }
    }

    count
  }

  /// Estimate memory usage of the tree
  fn estimate_memory_usage(&self) -> usize {
    let mut size = 0;

    for (name, node) in self.branch_nodes {
      size += name.len();
      size += node.name.len();
      size += node.parents.iter().map(|p| p.len()).sum::<usize>();
      size += node.children.iter().map(|c| c.len()).sum::<usize>();
      size += 64; // Estimate for struct overhead
    }

    size += self.cross_refs.len() * 64;
    size += self.visited.len() * 32;

    size
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

  use BranchMetadata;
  use insta::assert_snapshot;

  use super::*;

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
        commit_info: None,
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
        commit_info: None,
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
        commit_info: None,
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
        commit_info: None,
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
        commit_info: None,
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

  #[test]
  fn test_diamond_visualization() {
    let mut branches = HashMap::new();

    // Create a simple diamond: main -> feature1, feature2 -> merge
    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec!["merge".to_string()]),
    );
    branches.insert(
      "feature2".to_string(),
      create_test_branch("feature2", false, vec!["main".to_string()], vec!["merge".to_string()]),
    );
    branches.insert(
      "merge".to_string(),
      create_test_branch(
        "merge",
        false,
        vec!["feature1".to_string(), "feature2".to_string()],
        vec![],
      ),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let mut output = Vec::new();
    renderer.render_with_diamonds(&mut output, &roots, None, true).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    // Verify diamond symbols are present
    assert!(
      output_str.contains("◇") || output_str.contains("◅") || output_str.contains("◈"),
      "Expected diamond symbols in output: {}",
      output_str
    );

    // Verify all branches are rendered
    assert!(output_str.contains("main"));
    assert!(output_str.contains("feature1"));
    assert!(output_str.contains("feature2"));
    assert!(output_str.contains("merge"));
  }

  #[test]
  fn test_diamond_vs_regular_rendering() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec!["merge".to_string()]),
    );
    branches.insert(
      "feature2".to_string(),
      create_test_branch("feature2", false, vec!["main".to_string()], vec!["merge".to_string()]),
    );
    branches.insert(
      "merge".to_string(),
      create_test_branch(
        "merge",
        false,
        vec!["feature1".to_string(), "feature2".to_string()],
        vec![],
      ),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let mut regular_output = Vec::new();
    renderer.render(&mut regular_output, &roots, None).unwrap();
    let regular_str = String::from_utf8(regular_output).unwrap();

    renderer.visited.clear(); // Reset for second rendering
    let mut diamond_output = Vec::new();
    renderer.render_with_diamonds(&mut diamond_output, &roots, None, true).unwrap();
    let diamond_str = String::from_utf8(diamond_output).unwrap();

    // They should be different (diamond rendering should have special symbols)
    assert_ne!(regular_str, diamond_str, "Diamond and regular rendering should be different");

    // Both should contain all branch names
    for branch in ["main", "feature1", "feature2", "merge"] {
      assert!(regular_str.contains(branch), "Regular output should contain {}", branch);
      assert!(diamond_str.contains(branch), "Diamond output should contain {}", branch);
    }
  }

  #[test]
  fn test_circular_dependency_detection() {
    let mut branches = HashMap::new();

    // Create a circular dependency: A -> B -> C -> A
    branches.insert(
      "A".to_string(),
      create_test_branch("A", false, vec!["C".to_string()], vec!["B".to_string()]),
    );
    branches.insert(
      "B".to_string(),
      create_test_branch("B", false, vec!["A".to_string()], vec!["C".to_string()]),
    );
    branches.insert(
      "C".to_string(),
      create_test_branch("C", false, vec!["B".to_string()], vec!["A".to_string()]),
    );

    let roots = vec!["A".to_string()];
    let renderer = TreeRenderer::new(&branches, &roots, None, true);

    let circular_deps = renderer.detect_circular_dependencies();
    assert!(!circular_deps.is_empty(), "Should detect circular dependency");

    let cycle = &circular_deps[0];
    assert!(cycle.len() >= 3, "Cycle should contain at least 3 branches");
    assert!(cycle.contains(&"A".to_string()));
    assert!(cycle.contains(&"B".to_string()));
    assert!(cycle.contains(&"C".to_string()));
  }

  #[test]
  fn test_enhanced_cross_reference_rendering() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec!["merge".to_string()]),
    );
    branches.insert(
      "feature2".to_string(),
      create_test_branch("feature2", false, vec!["main".to_string()], vec!["merge".to_string()]),
    );
    branches.insert(
      "merge".to_string(),
      create_test_branch(
        "merge",
        false,
        vec!["feature1".to_string(), "feature2".to_string()],
        vec![],
      ),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let mut output = Vec::new();
    renderer
      .render_with_enhanced_cross_refs(&mut output, &roots, None, true, Some(10))
      .unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(output_str.contains("Cross-references summary"));
    assert!(output_str.contains("merge"));
  }

  #[test]
  fn test_branch_reference_indicators() {
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
        vec!["sub1".to_string(), "sub2".to_string()],
      ),
    );
    branches.insert(
      "sub1".to_string(),
      create_test_branch("sub1", false, vec!["feature1".to_string()], vec!["common".to_string()]),
    );
    branches.insert(
      "sub2".to_string(),
      create_test_branch("sub2", false, vec!["feature1".to_string()], vec!["common".to_string()]),
    );
    branches.insert(
      "common".to_string(),
      create_test_branch(
        "common",
        false,
        vec!["sub1".to_string(), "sub2".to_string()],
        vec![],
      ),
    );

    let roots = vec!["main".to_string()];
    let renderer = TreeRenderer::new(&branches, &roots, None, true);

    let common_refs = renderer.count_branch_references("common");
    assert_eq!(common_refs, 2, "Common branch should have 2 references");

    let feature1_refs = renderer.count_branch_references("feature1");
    assert_eq!(feature1_refs, 1, "Feature1 branch should have 1 reference");
  }

  #[test]
  fn test_no_circular_dependency_in_normal_tree() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );
    branches.insert(
      "feature2".to_string(),
      create_test_branch("feature2", false, vec!["main".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];
    let renderer = TreeRenderer::new(&branches, &roots, None, true);

    let circular_deps = renderer.detect_circular_dependencies();
    assert!(circular_deps.is_empty(), "Normal tree should have no circular dependencies");
  }

  #[test]
  fn test_deep_nesting_basic_rendering() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["level1".to_string()]),
    );
    branches.insert(
      "level1".to_string(),
      create_test_branch("level1", false, vec!["main".to_string()], vec!["level2".to_string()]),
    );
    branches.insert(
      "level2".to_string(),
      create_test_branch("level2", false, vec!["level1".to_string()], vec!["level3".to_string()]),
    );
    branches.insert(
      "level3".to_string(),
      create_test_branch("level3", false, vec!["level2".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let config = DeepNestingConfig {
      max_depth: Some(10),
      max_branches_per_level: Some(50),
      enable_pagination: false,
      page_size: 10,
      enable_pruning: false,
      prune_threshold: 100,
      show_depth_indicators: true,
    };

    let mut output = Vec::new();
    let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(output_str.contains("main"));
    assert!(output_str.contains("level1"));
    assert!(output_str.contains("level2"));
    assert!(output_str.contains("level3"));

    assert!(output_str.contains("[depth:1]"));
    assert!(output_str.contains("[depth:2]"));
    assert!(output_str.contains("[depth:3]"));

    assert_eq!(stats.total_branches, 4);
    assert_eq!(stats.max_depth_reached, 3);
    assert_eq!(stats.branches_pruned, 0);
  }

  #[test]
  fn test_deep_nesting_with_depth_limit() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["level1".to_string()]),
    );
    branches.insert(
      "level1".to_string(),
      create_test_branch("level1", false, vec!["main".to_string()], vec!["level2".to_string()]),
    );
    branches.insert(
      "level2".to_string(),
      create_test_branch("level2", false, vec!["level1".to_string()], vec!["level3".to_string()]),
    );
    branches.insert(
      "level3".to_string(),
      create_test_branch("level3", false, vec!["level2".to_string()], vec!["level4".to_string()]),
    );
    branches.insert(
      "level4".to_string(),
      create_test_branch("level4", false, vec!["level3".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let config = DeepNestingConfig {
      max_depth: Some(2),
      max_branches_per_level: Some(50),
      enable_pagination: false,
      page_size: 10,
      enable_pruning: false,
      prune_threshold: 100,
      show_depth_indicators: true,
    };

    let mut output = Vec::new();
    let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(output_str.contains("truncated at depth 2"));
    assert!(output_str.contains("main"));
    assert!(output_str.contains("level1"));
    assert!(output_str.contains("level2"));
    assert!(!output_str.contains("level4") || output_str.contains("truncated"));
    assert!(stats.branches_pruned > 0);
  }

  #[test]
  fn test_deep_nesting_with_pagination() {
    let mut branches = HashMap::new();

    let mut children = Vec::new();
    for i in 1..=15 {
      let child_name = format!("child{}", i);
      children.push(child_name.clone());
      branches.insert(
        child_name.clone(),
        create_test_branch(&child_name, false, vec!["main".to_string()], vec![]),
      );
    }

    branches.insert("main".to_string(), create_test_branch("main", false, vec![], children));

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let config = DeepNestingConfig {
      max_depth: Some(10),
      max_branches_per_level: Some(50),
      enable_pagination: true,
      page_size: 5,
      enable_pruning: false,
      prune_threshold: 100,
      show_depth_indicators: true,
    };

    let mut output = Vec::new();
    let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(output_str.contains("Page 2") || output_str.contains("Page 3"));
    assert!(output_str.contains("child1"));
    assert!(output_str.contains("child15"));
    assert_eq!(stats.total_branches, 16);
  }

  #[test]
  fn test_deep_nesting_with_pruning() {
    let mut branches = HashMap::new();

    let mut children = Vec::new();
    for i in 1..=120 {
      let child_name = format!("child{}", i);
      children.push(child_name.clone());
      branches.insert(
        child_name.clone(),
        create_test_branch(&child_name, false, vec!["main".to_string()], vec![]),
      );
    }

    branches.insert("main".to_string(), create_test_branch("main", false, vec![], children));

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let config = DeepNestingConfig {
      max_depth: Some(10),
      max_branches_per_level: Some(50),
      enable_pagination: false,
      page_size: 10,
      enable_pruning: true,
      prune_threshold: 100,
      show_depth_indicators: true,
    };

    let mut output = Vec::new();
    let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(output_str.contains("Large tree detected") && output_str.contains("pruning"));
    assert_eq!(stats.total_branches, 121);
  }

  #[test]
  fn test_deep_nesting_with_circular_dependencies() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature".to_string()]),
    );
    branches.insert(
      "feature".to_string(),
      create_test_branch("feature", false, vec!["main".to_string()], vec!["hotfix".to_string()]),
    );
    branches.insert(
      "hotfix".to_string(),
      create_test_branch("hotfix", false, vec!["feature".to_string()], vec!["main".to_string()]),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let config = DeepNestingConfig {
      max_depth: Some(10),
      max_branches_per_level: Some(50),
      enable_pagination: false,
      page_size: 10,
      enable_pruning: false,
      prune_threshold: 100,
      show_depth_indicators: true,
    };

    let mut output = Vec::new();
    let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(output_str.contains("circular dependencies detected"));
    assert!(output_str.contains("[CIRCULAR]") || output_str.contains("🔄"));
    assert!(stats.circular_deps_detected > 0);
  }

  #[test]
  fn test_deep_nesting_memory_estimation() {
    let mut branches = HashMap::new();

    branches.insert(
      "main".to_string(),
      create_test_branch("main", false, vec![], vec!["feature1".to_string(), "feature2".to_string()]),
    );
    branches.insert(
      "feature1".to_string(),
      create_test_branch("feature1", false, vec!["main".to_string()], vec![]),
    );
    branches.insert(
      "feature2".to_string(),
      create_test_branch("feature2", false, vec!["main".to_string()], vec![]),
    );

    let roots = vec!["main".to_string()];
    let mut renderer = TreeRenderer::new(&branches, &roots, None, true);

    let config = DeepNestingConfig {
      max_depth: Some(10),
      max_branches_per_level: Some(50),
      enable_pagination: false,
      page_size: 10,
      enable_pruning: false,
      prune_threshold: 100,
      show_depth_indicators: true,
    };

    let mut output = Vec::new();
    let stats = renderer.render_with_deep_nesting(&mut output, &roots, &config).unwrap();

    let output_str = String::from_utf8(output).unwrap();

    assert!(stats.memory_usage_estimate > 0);
    assert!(stats.memory_usage_estimate < 10000);
    assert!(output_str.contains("Memory estimate:"));
  }

  fn create_test_branch(name: &str, is_current: bool, parents: Vec<String>, children: Vec<String>) -> BranchNode {
    BranchNode {
      name: name.to_string(),
      is_current,
      metadata: None,
      parents,
      children,
      commit_info: None,
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
      commit_info: None,
    }
  }
}
