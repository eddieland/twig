use std::collections::HashMap;
use std::io;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use git2::{BranchType, Repository as Git2Repository};

use crate::git::detect_current_repository;
use crate::tree_renderer::{BranchNode, TreeRenderer};
use crate::utils::output::{format_command, print_info, print_warning};
use crate::worktree::RepoState;

/// Build the tree subcommand
pub fn build_command() -> Command {
  Command::new("tree")
    .about("Show your branch tree with associated issues and PRs")
    .long_about(
      "Display local branches in a tree-like view with their associated Jira issues and GitHub PRs.\n\n\
            This command shows all local branches in the current repository along with\n\
            any associated Jira tickets and GitHub pull requests. This gives you a\n\
            bird's-eye view of your development tree, helping you track which branches\n\
            are linked to specific issues and PRs for better workflow management.",
    )
    .alias("t")
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
        .help("Maximum depth to display in the tree")
        .value_name("DEPTH")
        .value_parser(clap::value_parser!(u32)),
    )
    .arg(
      Arg::new("no-color")
        .long("no-color")
        .help("Disable colored output")
        .action(clap::ArgAction::SetTrue),
    )
    .arg(
      Arg::new("root")
        .long("root")
        .help("Root branch to start the tree from")
        .value_name("BRANCH")
        .default_value("main"),
    )
}

/// Handle the tree command
pub fn handle_command(tree_matches: &clap::ArgMatches) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(repo_arg) = tree_matches.get_one::<String>("repo") {
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

  // Get command line options
  let max_depth = tree_matches.get_one::<u32>("max-depth").copied();
  let no_color = tree_matches.get_flag("no-color");
  let root_branch = tree_matches.get_one::<String>("root").unwrap().clone();

  // Collect branch information and convert to BranchNode format
  let mut branch_nodes = HashMap::new();
  let mut all_branch_names = Vec::new();

  for branch_result in branches {
    let (branch, _) = branch_result?;
    if let Some(name) = branch.name()? {
      let branch_issue = repo_state.get_branch_issue_by_branch(name);

      all_branch_names.push(name.to_string());

      // Create branch node with dependencies
      let (parents, children) = if name == root_branch {
        // Root branch has no parents, all other branches as children
        let children = all_branch_names
          .iter()
          .filter(|&branch_name| branch_name != name)
          .cloned()
          .collect();
        (vec![], children)
      } else {
        // Non-root branches depend on the root
        (vec![root_branch.clone()], vec![])
      };

      let is_current = branch.is_head();
      let branch_node = BranchNode {
        name: name.to_string(),
        is_current,
        branch_issue: branch_issue.cloned(),
        parents,
        children,
      };

      branch_nodes.insert(name.to_string(), branch_node);
    }
  }

  // Now we need to update the root branch with all the children after collecting
  // all branches
  if let Some(root_node) = branch_nodes.get_mut(&root_branch) {
    root_node.children = all_branch_names
      .iter()
      .filter(|&branch_name| branch_name != &root_branch)
      .cloned()
      .collect();
  }

  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  // Sort branch names - current branch first, then alphabetically
  all_branch_names.sort_by(|a, b| {
    let a_node = &branch_nodes[a];
    let b_node = &branch_nodes[b];

    if a_node.is_current {
      std::cmp::Ordering::Less
    } else if b_node.is_current {
      std::cmp::Ordering::Greater
    } else {
      a.cmp(b)
    }
  });

  // Update the root branch's children to be in sorted order
  // We need to sort the children separately to avoid borrowing issues
  let mut sorted_children: Vec<String> = all_branch_names
    .iter()
    .filter(|&branch_name| branch_name != &root_branch)
    .cloned()
    .collect();

  // Sort children with the same logic
  sorted_children.sort_by(|a, b| {
    let a_node = &branch_nodes[a];
    let b_node = &branch_nodes[b];

    if a_node.is_current {
      std::cmp::Ordering::Less
    } else if b_node.is_current {
      std::cmp::Ordering::Greater
    } else {
      a.cmp(b)
    }
  });

  // Now update the root node with sorted children
  if let Some(root_node) = branch_nodes.get_mut(&root_branch) {
    root_node.children = sorted_children;
  }

  // Check if the specified root branch exists
  if !branch_nodes.contains_key(&root_branch) {
    print_warning(&format!("Root branch '{root_branch}' not found. Available branches:",));
    for name in &all_branch_names {
      println!("  {name}",);
    }
    return Ok(());
  }

  // Use the specified root branch
  let roots = vec![root_branch.clone()];

  // Create and configure the tree renderer
  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, max_depth, no_color);

  // Render the tree starting from the specified root
  let mut stdout = io::stdout();
  renderer.render_tree(&mut stdout, &root_branch, 0, &[])?;

  // Show summary and help text
  display_summary(&branch_nodes);

  Ok(())
}

pub fn display_summary(branch_nodes: &HashMap<String, BranchNode>) {
  let branches_with_issues = branch_nodes.values().filter(|node| node.branch_issue.is_some()).count();

  let branches_with_prs = branch_nodes
    .values()
    .filter(|node| {
      node
        .branch_issue
        .as_ref()
        .map(|issue| issue.github_pr.is_some())
        .unwrap_or(false)
    })
    .count();

  if branches_with_issues == 0 && branches_with_prs == 0 {
    println!();
    print_info("To associate branches with issues and PRs:");
    println!(
      "  • Link Jira issues: {}",
      format_command("twig jira branch link <issue-key>")
    );
    println!(
      "  • Link GitHub PRs: {}",
      format_command("twig github pr link <pr-url>")
    );
  }
}
