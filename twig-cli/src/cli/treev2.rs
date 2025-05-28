use std::collections::{HashMap, HashSet, VecDeque};
use std::io;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use git2::{BranchType, Repository as Git2Repository};

use super::tree::display_summary;
use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::tree_renderer::{BranchNode, TreeRenderer};
use crate::utils::output::print_warning;

/// Build the treev2 subcommand
pub fn build_command() -> Command {
  Command::new("treev2")
    .about("Show your branch tree with associated issues and PRs (v2)")
    .long_about(
      "Display local branches in a hierarchical tree view with their associated Jira issues and GitHub PRs.\n\n\
            This command shows branch relationships in a tree-like structure, including parent-child\n\
            relationships and cross-references for branches that appear in multiple locations.\n\
            This gives you a comprehensive view of your development tree, helping you track which branches\n\
            are linked to specific issues and PRs for better workflow management.",
    )
    .arg(
      Arg::new("repo")
        .long("repo")
        .short('r')
        .help("Path to a specific repository")
        .value_name("PATH"),
    )
    .arg(
      Arg::new("max-depth")
        .long("max-depth")
        .short('d')
        .help("Maximum depth of the tree to display")
        .value_name("DEPTH")
        .value_parser(clap::value_parser!(u32)),
    )
    .arg(
      Arg::new("no-color")
        .long("no-color")
        .help("Disable colored output")
        .action(clap::ArgAction::SetTrue),
    )
}

/// Handle the treev2 command
pub fn handle_command(treev2_matches: &clap::ArgMatches) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(repo_arg) = treev2_matches.get_one::<String>("repo") {
    std::path::PathBuf::from(repo_arg)
  } else {
    detect_current_repository().context("Not in a git repository")?
  };

  // Open the repository
  let repo =
    Git2Repository::open(&repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Load repository state
  let repo_state = RepoState::load(&repo_path).unwrap_or_default();

  // Get all local branches
  let branches = repo.branches(Some(BranchType::Local))?;

  // Collect branch information
  let mut branch_nodes = HashMap::new();
  let mut branch_info = HashMap::new();
  let mut root_branches = HashSet::new();

  // First pass: collect basic branch information
  for branch_result in branches {
    let (branch, _) = branch_result?;
    if let Some(name) = branch.name()? {
      let is_current = branch.is_head();
      let branch_issue = repo_state.get_branch_issue_by_branch(name);

      // Get the commit that the branch points to
      let commit = branch.get().peel_to_commit()?;

      // Create a BranchNode for this branch
      let node = BranchNode {
        name: name.to_string(),
        is_current,
        metadata: branch_issue.cloned(),
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

  // Second pass: determine parent-child relationships
  for (branch_name, (commit_id, _)) in &branch_info {
    // For each branch, find its parent branches
    let branch_commit = repo.find_commit(*commit_id)?;

    // If the branch has a parent commit, check which branches contain that parent
    if branch_commit.parent_count() > 0 {
      let parent_commit = branch_commit.parent(0)?;

      // Find branches that point to this parent commit or have this commit in their
      // history
      for (other_name, (other_id, _)) in &branch_info {
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

  // Get command-line options
  let max_depth = treev2_matches.get_one::<u32>("max-depth").copied();
  let no_color = treev2_matches.get_flag("no-color");

  // Define root branches (branches with no parents)
  let roots = vec!["main".to_string()];

  // Create the tree renderer
  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, max_depth, no_color);
  for root in &roots {
    renderer.render_tree(&mut io::stdout(), root, 0, &Vec::new())?;
  }

  // Show summary if no branches were displayed
  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  display_summary(&branch_nodes);

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::build_command;

  #[test]
  fn test_build_command() {
    let cmd = build_command();
    assert_eq!(cmd.get_name(), "treev2");
  }
}
