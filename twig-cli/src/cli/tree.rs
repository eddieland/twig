//! # Tree Command
//!
//! Derive-based implementation of the tree command for visualizing branch
//! dependency trees.

use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use git2::Repository as Git2Repository;
use tree_renderer::TreeRenderer;
use twig_core::output::{format_command, print_info, print_warning};
use twig_core::{detect_repository, tree_renderer};

use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;

/// Command for displaying branch dependency trees
#[derive(Args)]
pub struct TreeArgs {
  /// Path to a specific repository
  #[arg(short, long, value_name = "PATH")]
  pub repo: Option<String>,

  /// Maximum depth to display in the tree
  #[arg(short = 'd', long = "max-depth", value_name = "DEPTH")]
  pub max_depth: Option<u32>,

  /// Disable colored output
  #[arg(long = "no-color")]
  pub no_color: bool,
}

/// Handle the tree command
///
/// This function processes the `tree` command, which visualizes branch
/// dependencies in a tree structure. It retrieves the repository path,
/// loads the repository state, resolves user-defined dependencies, and
/// renders the branch tree.
pub(crate) fn handle_tree_command(tree: TreeArgs) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(repo_arg) = tree.repo {
    PathBuf::from(repo_arg)
  } else {
    detect_repository().context("Not in a git repository")?
  };

  // Open the repository
  let repo =
    Git2Repository::open(&repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Load repository state
  let repo_state = twig_core::state::RepoState::load(&repo_path).unwrap_or_default();

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
  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, tree.max_depth, tree.no_color);
  let mut stdout = io::stdout();
  renderer.render(&mut stdout, &roots, Some("\n"))?;

  // Display orphaned branches if any
  if !orphaned.is_empty() {
    display_orphaned_branches(&orphaned);
  }

  // Show summary and help text
  display_summary(&branch_nodes);

  Ok(())
}

fn display_summary(branch_nodes: &std::collections::HashMap<String, tree_renderer::BranchNode>) {
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

fn display_no_roots_warning(branch_nodes: &std::collections::HashMap<String, tree_renderer::BranchNode>) {
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
