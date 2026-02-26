//! # Cascade Command
//!
//! Derive-based implementation of the cascade command for performing a
//! cascading rebase from the current branch to its children.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use git2::Repository as Git2Repository;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::{RepoState, detect_repository};

use super::rebase_common::{
  ConflictResolution, RebaseContinueOutcome, RebaseResult, abort_rebase, attempt_rebase_continue, attempt_rebase_skip,
  execute_git_command, handle_rebase_conflict, rebase_branch, rebase_branch_force, show_dependency_tree,
};
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
  // Track branches that could not be rebased so that their descendants are also skipped.
  let mut failed_branches: HashSet<String> = HashSet::new();
  'branches: for branch in rebase_order {
    // Skip this branch if any of its parents failed to rebase — rebasing onto an
    // un-rebased parent would produce incorrect results.
    let dependency_parents = repo_state.get_dependency_parents(&branch);
    if dependency_parents.iter().any(|p| failed_branches.contains(*p)) {
      print_warning(&format!("Skipping {branch}: a parent branch could not be rebased"));
      failed_branches.insert(branch.clone());
      continue 'branches;
    }

    // Get the parents of this branch
    let parents = dependency_parents;

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
        let output = checkout_result.output.trim().to_string();
        if output.contains("is already used by worktree") {
          print_error(&format!(
            "Failed to checkout branch {branch}: {output}\n  \
             Hint: this branch is checked out in another worktree. \
             Switch to that worktree to rebase it, or use `git worktree remove` \
             to detach it first."
          ));
        } else {
          print_error(&format!("Failed to checkout branch {branch}: {output}"));
        }
        failed_branches.insert(branch.clone());
        continue 'branches;
      }

      // Execute the rebase
      let result = rebase_branch(repo_path, parent, autostash)?;

      match result {
        RebaseResult::Success => {
          print_success(&format!("Successfully rebased {branch} onto {parent}",));
        }
        RebaseResult::UpToDate => {
          if force {
            // Force rebase even if up-to-date
            print_info("Branch is up-to-date, but force flag is set. Rebasing anyway...");
            let force_result = rebase_branch_force(repo_path, parent, autostash)?;
            match force_result {
              RebaseResult::Success => {
                print_success(&format!("Successfully force-rebased {branch} onto {parent}",));
              }
              _ => {
                print_error(&format!("Failed to force-rebase {branch} onto {parent}",));
                failed_branches.insert(branch.clone());
                continue 'branches;
              }
            }
          } else {
            print_info(&format!("Branch {branch} is already up-to-date with {parent}",));
          }
        }
        RebaseResult::Conflict => {
          // Loop so that a second conflict arising after --continue or --skip re-prompts
          // the user rather than treating it as an unrecoverable error.
          'conflict_loop: loop {
            print_warning(&format!("Conflicts detected while rebasing {branch} onto {parent}",));
            let resolution = handle_rebase_conflict()?;

            match resolution {
              ConflictResolution::Continue => match attempt_rebase_continue(repo_path)? {
                RebaseContinueOutcome::Completed => {
                  print_success(&format!(
                    "Rebase of {branch} onto {parent} completed after resolving conflicts",
                  ));
                  break 'conflict_loop;
                }
                RebaseContinueOutcome::MoreConflicts => continue 'conflict_loop,
                RebaseContinueOutcome::Failed => {
                  print_error(&format!(
                    "Failed to continue rebase of {branch} onto {parent}. \
                     You may need to resolve conflicts manually."
                  ));
                  abort_rebase(repo_path)?;
                  failed_branches.insert(branch.clone());
                  continue 'branches;
                }
              },
              ConflictResolution::AbortToOriginal => {
                abort_rebase(repo_path)?;
                print_info(&format!("Rebase of {branch} onto {parent} aborted",));

                // Checkout the original branch
                let checkout_result = execute_git_command(repo_path, &["checkout", &current_branch_name])?;
                if !checkout_result.output.is_empty() {
                  print_info(&checkout_result.output);
                }

                return Ok(());
              }
              ConflictResolution::AbortStayHere => {
                abort_rebase(repo_path)?;
                print_info(&format!("Rebase of {branch} onto {parent} aborted",));
                failed_branches.insert(branch.clone());
                continue 'branches;
              }
              ConflictResolution::Skip => match attempt_rebase_skip(repo_path)? {
                RebaseContinueOutcome::Completed => {
                  print_info(&format!("Skipped commit during rebase of {branch} onto {parent}",));
                  break 'conflict_loop;
                }
                RebaseContinueOutcome::MoreConflicts => continue 'conflict_loop,
                RebaseContinueOutcome::Failed => {
                  print_error(&format!(
                    "Failed to skip commit during rebase of {branch} onto {parent}. \
                     You may need to resolve conflicts manually."
                  ));
                  abort_rebase(repo_path)?;
                  failed_branches.insert(branch.clone());
                  continue 'branches;
                }
              },
            }
          }
        }
        RebaseResult::Error => {
          print_error(&format!("Failed to rebase {branch} onto {parent}",));
          // Skip this branch's descendants since the rebase did not complete.
          failed_branches.insert(branch.clone());
          continue 'branches;
        }
      }
    }
  }

  // Return to the original branch
  let checkout_result = execute_git_command(repo_path, &["checkout", &current_branch_name])?;
  if !checkout_result.output.is_empty() {
    print_info(&checkout_result.output);
  }

  if !failed_branches.is_empty() {
    print_warning(&format!(
      "{} branch{} could not be rebased and {} skipped (along with any dependents):",
      failed_branches.len(),
      if failed_branches.len() == 1 { "" } else { "es" },
      if failed_branches.len() == 1 { "was" } else { "were" },
    ));
    let mut sorted: Vec<&String> = failed_branches.iter().collect();
    sorted.sort();
    for b in sorted {
      print_warning(&format!("  - {b}"));
    }
    print_warning("Cascading rebase completed with errors");
  } else {
    print_success("Cascading rebase completed successfully");
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
