//! # Cascade Command
//!
//! Derive-based implementation of the cascade command for performing a
//! cascading rebase from the current branch to its children.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;
use git2::Repository as Git2Repository;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::tree_renderer::TreeRenderer;
use twig_core::{RepoState, detect_repository, twig_theme};

use crate::consts;
use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;

/// Command for performing a cascading rebase
#[derive(Args)]
pub struct CascadeArgs {
  /// Maximum depth for cascading rebase
  #[arg(long = "max-depth", value_name = "DEPTH")]
  pub max_depth: Option<u32>,

  /// Force rebase even if branches are up-to-date
  #[arg(long)]
  pub force: bool,

  /// Show dependency graph before rebasing
  #[arg(long = "show-graph")]
  pub show_graph: bool,

  /// Automatically stash and pop pending changes
  #[arg(long)]
  pub autostash: bool,

  /// Show the rebase plan without executing it
  #[arg(long)]
  pub preview: bool,

  /// Path to a specific repository
  #[arg(short, long, value_name = "PATH")]
  pub repo: Option<String>,
}

/// Handle the cascade command
pub fn handle_cascade_command(args: CascadeArgs) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(ref repo_arg) = args.repo {
    PathBuf::from(repo_arg)
  } else {
    detect_repository().context("Not in a git repository")?
  };

  // Extract the values we need from args
  let max_depth = args.max_depth;
  let force = args.force;
  let show_graph = args.show_graph;
  let autostash = args.autostash;
  let preview = args.preview;

  // Perform cascading rebase from current branch to children
  rebase_downstream(&repo_path, max_depth, force, show_graph, autostash, preview)
}

/// Perform cascading rebase from current branch to children
fn rebase_downstream(
  repo_path: &Path,
  max_depth: Option<u32>,
  force: bool,
  show_graph: bool,
  autostash: bool,
  preview: bool,
) -> Result<()> {
  // Open the repository
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the current branch
  let head = repo.head()?;
  if !head.is_branch() {
    return Err(anyhow::anyhow!("HEAD is not a branch. Cannot cascade rebase."));
  }

  let current_branch_name = head.shorthand().unwrap_or("HEAD").to_string();
  print_info(&format!("Current branch: {current_branch_name}",));

  // Load repository state
  let repo_state = RepoState::load(repo_path).unwrap_or_default();

  // Create the user-defined dependency resolver
  let resolver = UserDefinedDependencyResolver;

  // Build the branch node tree structure
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;

  // Check if we have any branches at all
  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  // Show dependency graph if requested
  if show_graph {
    show_dependency_tree(repo_path, &current_branch_name)?;
  }

  // Get all children of the current branch
  let children = get_all_descendants(&repo_state, &current_branch_name, max_depth);

  if children.is_empty() {
    print_warning("No child branches found for the current branch.");
    return Ok(());
  }

  print_info(&format!("Found {} child branches to rebase", children.len()));

  // Build a dependency graph to determine the order of rebasing
  let rebase_order = determine_rebase_order(&repo_state, &current_branch_name, &children);

  // Preview mode: show the plan without executing
  if preview {
    println!();
    print_info(&format!(
      "Would rebase {} branch{}:",
      rebase_order.len(),
      if rebase_order.len() == 1 { "" } else { "es" }
    ));
    for branch in &rebase_order {
      let parents = repo_state.get_dependency_parents(branch);
      for parent in parents {
        println!("  {} onto {}", branch, parent);
      }
    }
    println!();
    show_dependency_tree(repo_path, &current_branch_name)?;
    return Ok(());
  }

  // Perform the cascading rebase
  for branch in rebase_order {
    // Get the parents of this branch
    let parents = repo_state.get_dependency_parents(&branch);

    if parents.is_empty() {
      print_warning(&format!("No parent branches found for {branch}, skipping",));
      continue;
    }

    // Rebase this branch onto each of its parents
    for parent in parents {
      print_info(&format!("Rebasing {branch} onto {parent}"));

      // First checkout the branch
      let checkout_result = execute_git_command(repo_path, &["checkout", &branch])?;
      if !checkout_result.success {
        print_error(&format!(
          "Failed to checkout branch {branch}: {}",
          checkout_result.output
        ));
        continue;
      }

      // Execute the rebase
      let result = rebase_branch(repo_path, &branch, parent, autostash)?;

      match result {
        RebaseResult::Success => {
          print_success(&format!("Successfully rebased {branch} onto {parent}",));
        }
        RebaseResult::UpToDate => {
          if force {
            // Force rebase even if up-to-date
            print_info("Branch is up-to-date, but force flag is set. Rebasing anyway...");
            let force_result = rebase_branch_force(repo_path, &branch, parent, autostash)?;
            match force_result {
              RebaseResult::Success => {
                print_success(&format!("Successfully force-rebased {branch} onto {parent}",));
              }
              _ => {
                print_error(&format!("Failed to force-rebase {branch} onto {parent}",));
                // Continue with other branches rather than aborting the whole process
                continue;
              }
            }
          } else {
            print_info(&format!("Branch {branch} is already up-to-date with {parent}",));
          }
        }
        RebaseResult::Conflict => {
          // Handle conflict
          print_warning(&format!("Conflicts detected while rebasing {branch} onto {parent}",));
          let resolution = handle_rebase_conflict(repo_path, &branch)?;

          match resolution {
            ConflictResolution::Continue => {
              // Continue the rebase
              let continue_result = execute_git_command(repo_path, &["rebase", "--continue"])?;
              print_info(&continue_result.output);
              print_success(&format!(
                "Rebase of {branch} onto {parent} completed after resolving conflicts",
              ));
            }
            ConflictResolution::AbortToOriginal => {
              // Abort the rebase and go back to the original branch
              let abort_result = execute_git_command(repo_path, &["rebase", "--abort"])?;
              print_info(&abort_result.output);
              print_info(&format!("Rebase of {branch} onto {parent} aborted",));

              // Checkout the original branch
              let checkout_result = execute_git_command(repo_path, &["checkout", &current_branch_name])?;
              print_info(&checkout_result.output);

              return Ok(());
            }
            ConflictResolution::AbortStayHere => {
              // Abort the rebase but stay on the current branch
              let abort_result = execute_git_command(repo_path, &["rebase", "--abort"])?;
              print_info(&abort_result.output);
              print_info(&format!("Rebase of {branch} onto {parent} aborted",));
              continue;
            }
            ConflictResolution::Skip => {
              // Skip the current commit
              let skip_result = execute_git_command(repo_path, &["rebase", "--skip"])?;
              print_info(&skip_result.output);
              print_info(&format!("Skipped commit during rebase of {branch} onto {parent}",));
            }
          }
        }
        RebaseResult::Error => {
          print_error(&format!("Failed to rebase {branch} onto {parent}",));
          // Continue with other branches rather than aborting the whole process
          continue;
        }
      }
    }
  }

  // Return to the original branch
  let checkout_result = execute_git_command(repo_path, &["checkout", &current_branch_name])?;
  print_info(&checkout_result.output);

  print_success("Cascading rebase completed successfully");
  Ok(())
}

/// Show the dependency tree
fn show_dependency_tree(repo_path: &Path, _current_branch: &str) -> Result<()> {
  // Open the repository
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Load repository state
  let repo_state = RepoState::load(repo_path).unwrap_or_default();

  // Create the user-defined dependency resolver
  let resolver = UserDefinedDependencyResolver;

  // Build the branch node tree structure
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;

  // Get root branches and orphaned branches for the tree
  let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

  if roots.is_empty() {
    print_warning("No root branches found. Cannot display dependency tree.");
    return Ok(());
  }

  print_info("Branch dependency tree:");

  // Create and configure the tree renderer
  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, None, false);

  // Render all root trees
  let mut stdout = std::io::stdout();
  for (i, root) in roots.iter().enumerate() {
    if i > 0 {
      println!(); // Add spacing between multiple trees
    }
    renderer.render_tree(&mut stdout, root, 0, &[], false)?;
  }

  // Display orphaned branches if any
  if !orphaned.is_empty() {
    println!("\n📝 Orphaned branches (no dependencies defined):");
    for branch in orphaned {
      println!("  • {branch}",);
    }
  }

  Ok(())
}

/// Get all descendants of a branch up to a certain depth
fn get_all_descendants(repo_state: &RepoState, branch: &str, max_depth: Option<u32>) -> Vec<String> {
  let mut descendants = Vec::new();
  let mut visited = HashSet::new();
  let mut queue = VecDeque::new();

  // Start with the immediate children
  let children = repo_state.get_dependency_children(branch);
  for child in children {
    queue.push_back((child.to_string(), 1));
  }

  while let Some((current, depth)) = queue.pop_front() {
    // Check if we've reached the maximum depth
    if let Some(max) = max_depth
      && depth > max
    {
      continue;
    }

    // Add to descendants if not already visited
    if !visited.contains(&current) {
      descendants.push(current.clone());
      visited.insert(current.clone());

      // Add children to the queue
      let children = repo_state.get_dependency_children(&current);
      for child in children {
        queue.push_back((child.to_string(), depth + 1));
      }
    }
  }

  descendants
}

/// Determine the order in which branches should be rebased
fn determine_rebase_order(repo_state: &RepoState, start_branch: &str, branches: &[String]) -> Vec<String> {
  // Build a dependency graph
  let mut graph: HashMap<String, Vec<String>> = HashMap::new();

  // Pre-build HashSet for O(1) membership checks instead of O(n) Vec lookups
  let branch_set: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();

  // Add all branches to the graph
  for branch in branches {
    graph.insert(branch.clone(), Vec::new());
  }

  // Add dependencies
  for branch in branches {
    let parents = repo_state.get_dependency_parents(branch);
    for parent in parents {
      if branch_set.contains(parent) {
        // If the parent is also in our list of branches to rebase,
        // add it as a dependency (child depends on parent)
        if let Some(deps) = graph.get_mut(parent) {
          deps.push(branch.clone());
        }
      }
    }
  }

  // Perform a topological sort
  let mut result = Vec::new();
  let mut visited = HashSet::new();
  let mut temp_visited = HashSet::new();

  fn visit(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    temp_visited: &mut HashSet<String>,
    result: &mut Vec<String>,
  ) {
    if temp_visited.contains(node) {
      // Cycle detected, skip this node
      return;
    }

    if !visited.contains(node) {
      temp_visited.insert(node.to_string());

      if let Some(deps) = graph.get(node) {
        for dep in deps {
          visit(dep, graph, visited, temp_visited, result);
        }
      }

      temp_visited.remove(node);
      visited.insert(node.to_string());
      result.push(node.to_string());
    }
  }

  // Start with branches that depend on the start branch
  let start_children = repo_state.get_dependency_children(start_branch);
  for child in start_children {
    if branch_set.contains(child) {
      visit(child, &graph, &mut visited, &mut temp_visited, &mut result);
    }
  }

  // Process any remaining branches
  for branch in branches {
    if !visited.contains(branch) {
      visit(branch, &graph, &mut visited, &mut temp_visited, &mut result);
    }
  }

  result
}

/// Enum representing rebase result
enum RebaseResult {
  Success,
  UpToDate,
  Conflict,
  Error,
}

/// Enum representing rebase conflict resolution options
enum ConflictResolution {
  Continue,
  AbortToOriginal,
  AbortStayHere,
  Skip,
}

/// Rebase a branch onto another branch
fn rebase_branch(repo_path: &Path, _branch: &str, onto: &str, autostash: bool) -> Result<RebaseResult> {
  // Build the rebase command
  let mut args = vec!["rebase"];

  if autostash {
    args.push("--autostash");
  }

  args.push(onto);

  // Execute the rebase command
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(&args)
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git rebase command")?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  // Print output
  if !stdout.is_empty() {
    print_info(&stdout);
  }

  if !stderr.is_empty() {
    // Check for specific error messages
    if stderr.contains("up to date") || stdout.contains("up to date") {
      return Ok(RebaseResult::UpToDate);
    }

    if stderr.contains("CONFLICT") || stdout.contains("CONFLICT") {
      return Ok(RebaseResult::Conflict);
    }

    print_warning(&stderr);
  }

  if output.status.success() {
    Ok(RebaseResult::Success)
  } else {
    Ok(RebaseResult::Error)
  }
}

/// Force rebase a branch onto another branch (used with --force flag)
fn rebase_branch_force(repo_path: &Path, _branch: &str, onto: &str, autostash: bool) -> Result<RebaseResult> {
  // Build the rebase command with --force-rebase
  let mut args = vec!["rebase", "--force-rebase"];

  if autostash {
    args.push("--autostash");
  }

  args.push(onto);

  // Execute the rebase command
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(&args)
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git rebase command")?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  // Print output
  if !stdout.is_empty() {
    print_info(&stdout);
  }

  if !stderr.is_empty() {
    // Check for specific error messages
    if stderr.contains("CONFLICT") || stdout.contains("CONFLICT") {
      return Ok(RebaseResult::Conflict);
    }

    print_warning(&stderr);
  }

  if output.status.success() {
    Ok(RebaseResult::Success)
  } else {
    Ok(RebaseResult::Error)
  }
}

/// Handle rebase conflicts
fn handle_rebase_conflict(_repo_path: &Path, _branch: &str) -> Result<ConflictResolution> {
  print_info("Rebase conflict detected. You have several options:");
  println!();

  let choice = dialoguer::Select::with_theme(&twig_theme())
    .with_prompt("Select an option")
    .items([
      "Continue - Resolve conflicts and continue the rebase",
      "Abort to original - Abort the rebase and return to the original branch",
      "Abort stay here - Abort the rebase but stay on the current branch",
      "Skip - Skip the current commit and continue",
    ])
    .default(0)
    .interact()
    .unwrap_or(0);

  match choice {
    0 => Ok(ConflictResolution::Continue),
    1 => Ok(ConflictResolution::AbortToOriginal),
    2 => Ok(ConflictResolution::AbortStayHere),
    3 => Ok(ConflictResolution::Skip),
    _ => Ok(ConflictResolution::AbortToOriginal),
  }
}

/// Output from a git command, including both the combined stdout/stderr text and
/// whether the process exited successfully (exit code 0).
struct GitCommandOutput {
  /// Combined stdout and stderr text.
  output: String,
  /// Whether the command exited with status code 0.
  success: bool,
}

/// Execute a git command and return its output along with the exit status.
fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<GitCommandOutput> {
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(args)
    .current_dir(repo_path)
    .output()
    .context(format!("Failed to execute git command: {args:?}",))?;

  let success = output.status.success();
  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  let mut combined = String::new();

  if !stdout.is_empty() {
    combined.push_str(&stdout);
  }

  if !stderr.is_empty() {
    if !combined.is_empty() {
      combined.push('\n');
    }
    combined.push_str(&stderr);
  }

  Ok(GitCommandOutput {
    output: combined,
    success,
  })
}
