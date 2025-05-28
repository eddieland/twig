//! # Tree Command
//!
//! CLI command for visualizing branch dependency trees, showing hierarchical
//! relationships between branches with optional depth limits and formatting
//! options.

use std::io;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use git2::Repository as Git2Repository;

use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::tree_renderer::TreeRenderer;
use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;
use crate::utils::output::{format_command, print_info, print_warning};

/// Build the tree subcommand
pub fn build_command() -> Command {
  Command::new("tree")
    .about("Show your branch tree with user-defined dependencies")
    .long_about(
      "Display local branches in a tree-like view based on user-defined dependencies.\n\n\
            This command shows branch relationships that you have explicitly defined using\n\
            the 'twig branch depend' command. It also displays associated Jira issues and\n\
            GitHub PRs. Branches without defined dependencies or root status will be shown\n\
            as orphaned branches. Use 'twig branch depend' to create relationships and\n\
            'twig branch root add' to designate root branches.",
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

  // Get command line options
  let max_depth = tree_matches.get_one::<u32>("max-depth").copied();
  let no_color = tree_matches.get_flag("no-color");

  // Create the user-defined dependency resolver
  let resolver = UserDefinedDependencyResolver;

  // Build the branch node tree structure
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;

  // Check if we have any branches at all
  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  // Check if we have any user-defined dependencies
  let has_dependencies = repo_state.has_user_defined_dependencies();
  let has_root_branches = !repo_state.get_root_branches().is_empty();

  if !has_dependencies && !has_root_branches {
    display_empty_state_help();
    return Ok(());
  }

  // Get root branches and orphaned branches for the tree
  let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

  if roots.is_empty() {
    display_no_roots_warning(&branch_nodes);
    return Ok(());
  }

  // Create and configure the tree renderer
  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, max_depth, no_color);

  // Render all root trees
  let mut stdout = io::stdout();
  for (i, root) in roots.iter().enumerate() {
    if i > 0 {
      println!(); // Add spacing between multiple trees
    }
    renderer.render_tree(&mut stdout, root, 0, &[], false)?;
  }

  // Display orphaned branches if any
  if !orphaned.is_empty() {
    display_orphaned_branches(&orphaned);
  }

  // Show summary and help text
  display_summary(&branch_nodes);

  Ok(())
}

pub fn display_summary(branch_nodes: &std::collections::HashMap<String, crate::tree_renderer::BranchNode>) {
  let branches_with_issues = branch_nodes.values().filter(|node| node.metadata.is_some()).count();

  let branches_with_prs = branch_nodes
    .values()
    .filter(|node| {
      node
        .metadata
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

fn display_empty_state_help() {
  print_info("No user-defined dependencies or root branches found.");
  println!("\nTo get started with branch dependencies:");
  println!(
    "  • Define root branches: {}",
    format_command("twig branch root add <branch-name>")
  );
  println!(
    "  • Add dependencies: {}",
    format_command("twig branch depend <parent-branch>")
  );
  println!("  • View current setup: {}", format_command("twig branch list"));
  println!("\nThis will create a tree structure showing your branch relationships.");
}

fn display_no_roots_warning(branch_nodes: &std::collections::HashMap<String, crate::tree_renderer::BranchNode>) {
  print_warning("Found user-defined dependencies but no root branches.");

  let branch_names: Vec<&String> = branch_nodes.keys().collect();
  println!("\nAvailable branches:");
  for name in &branch_names {
    println!("  {name}");
  }

  println!("\nTo fix this, designate one or more root branches:");
  println!("  {}", format_command("twig branch root add <branch-name>"));
}

fn display_orphaned_branches(orphaned: &[String]) {
  println!("\n📝 Orphaned branches (no dependencies defined):");
  for branch in orphaned {
    println!("  • {branch}");
  }

  println!("\nTo organize these branches:");
  println!(
    "  • Add as root: {}",
    format_command("twig branch root add <branch-name>")
  );
  println!(
    "  • Add dependency: {}",
    format_command("twig branch depend <parent-branch>")
  );
}
