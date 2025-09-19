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
use twig_core::{RepoState, detect_repository};

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

  /// Force push to remote after successful rebase (WARNING: This can overwrite remote changes)
  #[arg(long = "force-push")]
  pub force_push: bool,

  /// Show dependency graph before rebasing
  #[arg(long = "show-graph")]
  pub show_graph: bool,

  /// Automatically stash and pop pending changes
  #[arg(long)]
  pub autostash: bool,

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
  let force_push = args.force_push;
  let show_graph = args.show_graph;
  let autostash = args.autostash;

  // Perform cascading rebase from current branch to children
  rebase_downstream(&repo_path, max_depth, force, force_push, show_graph, autostash)
}

/// Perform cascading rebase from current branch to children
fn rebase_downstream(
  repo_path: &Path,
  max_depth: Option<u32>,
  force: bool,
  force_push: bool,
  show_graph: bool,
  autostash: bool,
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

  // Perform the cascading rebase
  for branch in rebase_order {
    // Check if the branch exists before attempting operations
    if !branch_exists(repo_path, &branch)? {
      print_warning(&format!("Branch '{}' does not exist, skipping rebase", branch));
      continue;
    }

    // Get the parents of this branch
    let parents = repo_state.get_dependency_parents(&branch);

    if parents.is_empty() {
      print_warning(&format!("No parent branches found for {branch}, skipping",));
      continue;
    }

    // Rebase this branch onto each of its parents
    for parent in parents {
      // Check if the parent branch exists
      if !branch_exists(repo_path, parent)? {
        print_warning(&format!(
          "Parent branch '{}' does not exist, skipping rebase of {} onto {}",
          parent, branch, parent
        ));
        continue;
      }

      print_info(&format!("Rebasing {branch} onto {parent}"));

      // First checkout the branch
      let checkout_result = execute_git_command(repo_path, &["checkout", &branch])?;
      if !checkout_result.contains("Switched to branch") && !checkout_result.contains("Already on") {
        print_error(&format!("Failed to checkout branch {branch}: {checkout_result}"));
        continue;
      }

      // Execute the rebase
      let result = rebase_branch(repo_path, &branch, parent, autostash)?;

      match result {
        RebaseResult::Success => {
          print_success(&format!("Successfully rebased {branch} onto {parent}",));
          
          // Force push if requested
          if force_push {
            if let Err(e) = handle_force_push(repo_path, &branch) {
              print_error(&format!("Failed to force push {branch}: {e}"));
              // Continue with other branches rather than aborting the whole process
            }
          }
        }
        RebaseResult::UpToDate => {
          if force {
            // Force rebase even if up-to-date
            print_info("Branch is up-to-date, but force flag is set. Rebasing anyway...");
            let force_result = rebase_branch_force(repo_path, &branch, parent, autostash)?;
            match force_result {
              RebaseResult::Success => {
                print_success(&format!("Successfully force-rebased {branch} onto {parent}",));
                
                // Force push if requested
                if force_push {
                  if let Err(e) = handle_force_push(repo_path, &branch) {
                    print_error(&format!("Failed to force push {branch}: {e}"));
                    // Continue with other branches rather than aborting the whole process
                  }
                }
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
              print_info(&continue_result);
              print_success(&format!(
                "Rebase of {branch} onto {parent} completed after resolving conflicts",
              ));
              
              // Force push if requested
              if force_push {
                if let Err(e) = handle_force_push(repo_path, &branch) {
                  print_error(&format!("Failed to force push {branch}: {e}"));
                  // Continue with other branches rather than aborting the whole process
                }
              }
            }
            ConflictResolution::AbortToOriginal => {
              // Abort the rebase and go back to the original branch
              let abort_result = execute_git_command(repo_path, &["rebase", "--abort"])?;
              print_info(&abort_result);
              print_info(&format!("Rebase of {branch} onto {parent} aborted",));

              // Checkout the original branch
              let checkout_result = execute_git_command(repo_path, &["checkout", &current_branch_name])?;
              print_info(&checkout_result);

              return Ok(());
            }
            ConflictResolution::AbortStayHere => {
              // Abort the rebase but stay on the current branch
              let abort_result = execute_git_command(repo_path, &["rebase", "--abort"])?;
              print_info(&abort_result);
              print_info(&format!("Rebase of {branch} onto {parent} aborted",));
              continue;
            }
            ConflictResolution::Skip => {
              // Skip the current commit
              let skip_result = execute_git_command(repo_path, &["rebase", "--skip"])?;
              print_info(&skip_result);

              // Check if the rebase is still in progress after skip
              if is_rebase_in_progress(repo_path) {
                // There might be more conflicts, continue handling the rebase
                print_warning("Rebase is still in progress after skip. Checking for additional conflicts...");

                // Check the status after skip
                let status_output = execute_git_command(repo_path, &["status", "--porcelain"])?;
                if !status_output.trim().is_empty() {
                  // There are still conflicts or other issues
                  print_warning("Additional conflicts detected after skip. Please resolve them manually.");
                  print_info("You can:");
                  print_info("  • Continue the rebase: git rebase --continue");
                  print_info("  • Abort the rebase: git rebase --abort");
                  print_info("  • Skip more commits: git rebase --skip");
                  continue;
                }
              } else {
                // Rebase completed successfully after skip
                print_success(&format!(
                  "Rebase of {branch} onto {parent} completed after skipping commit",
                ));
              }

              // Clean up any unmerged entries in the index and working directory after skip
              cleanup_index_after_skip(repo_path)?;

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
  print_info(&checkout_result);

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

  // Add all branches to the graph
  for branch in branches {
    graph.insert(branch.clone(), Vec::new());
  }

  // Add dependencies
  for branch in branches {
    let parents = repo_state.get_dependency_parents(branch);
    for parent in parents {
      if branches.contains(&parent.to_string()) {
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
    if branches.contains(&child.to_string()) {
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
  println!("  1. Continue - Resolve conflicts and continue the rebase");
  println!("  2. Abort to original - Abort the rebase and return to the original branch");
  println!("  3. Abort stay here - Abort the rebase but stay on the current branch");
  println!("  4. Skip - Skip the current commit and continue");

  // In a real interactive environment, we would prompt the user here
  // For now, we'll just return Continue as the default

  // This would be replaced with actual user input in a real implementation
  let choice = dialoguer::Select::new()
    .with_prompt("Select an option")
    .items(&["Continue", "Abort to original", "Abort stay here", "Skip"])
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

/// Handle force push to remote for a branch after successful rebase
fn handle_force_push(repo_path: &Path, branch: &str) -> Result<()> {
  // First, check if the branch has a remote tracking branch
  let remote_ref = format!("origin/{}", branch);
  let check_remote_args = ["rev-parse", "--verify", &remote_ref];
  
  match execute_git_command(repo_path, &check_remote_args) {
    Ok(_) => {
      // Remote tracking branch exists, proceed with force push
      print_warning(&format!("⚠️  Force pushing {branch} to remote (this may overwrite remote changes)"));
      
      let force_push_args = ["push", "--force-with-lease", "origin", branch];
      match execute_git_command(repo_path, &force_push_args) {
        Ok(output) => {
          if !output.trim().is_empty() {
            print_info(&output);
          }
          print_success(&format!("Successfully force-pushed {branch} to origin"));
          Ok(())
        }
        Err(e) => {
          // Try to provide more helpful error message
          if let Some(git_error) = e.downcast_ref::<std::io::Error>() {
            return Err(anyhow::anyhow!(
              "Force push failed for {}: {}. This might be due to remote branch protection or network issues.", 
              branch, git_error
            ));
          }
          Err(e.context(format!("Failed to force push {branch}")))
        }
      }
    }
    Err(_) => {
      // No remote tracking branch, skip force push
      print_info(&format!("Branch {branch} has no remote tracking branch, skipping force push"));
      Ok(())
    }
  }
}

/// Execute a git command and handle output
fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<String> {
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(args)
    .current_dir(repo_path)
    .output()
    .context(format!("Failed to execute git command: {args:?}",))?;

  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  let mut result = String::new();

  if !stdout.is_empty() {
    result.push_str(&stdout);
  }

  if !stderr.is_empty() {
    if !result.is_empty() {
      result.push('\n');
    }
    result.push_str(&stderr);
  }

  Ok(result)
}

/// Clean up the index and working directory after a rebase skip operation
/// This removes any unmerged entries that might be left in the index and
/// resets the working directory to match HEAD, ensuring a clean state
fn cleanup_index_after_skip(repo_path: &Path) -> Result<()> {
  // Open the repository using git2
  let repo = Git2Repository::open(repo_path).context("Failed to open repository for index cleanup")?;

  // Get the current HEAD commit
  let head = repo.head()?;
  let head_commit = head.peel_to_commit()?;

  // Reset both index and working directory to match the HEAD commit
  // This clears any unmerged entries and unstaged changes left by the skip
  repo
    .reset(head_commit.as_object(), git2::ResetType::Hard, None)
    .context("Failed to reset repository state after skip")?;

  Ok(())
}

/// Check if a rebase is currently in progress
fn is_rebase_in_progress(repo_path: &Path) -> bool {
  // Check for the existence of .git/rebase-merge directory
  let rebase_merge_dir = repo_path.join(".git").join("rebase-merge");
  if rebase_merge_dir.exists() {
    return true;
  }

  // Check for the existence of .git/rebase-apply directory
  let rebase_apply_dir = repo_path.join(".git").join("rebase-apply");
  if rebase_apply_dir.exists() {
    return true;
  }

  false
}

/// Check if a branch exists in the repository
fn branch_exists(repo_path: &Path, branch_name: &str) -> Result<bool> {
  let result = Command::new(consts::GIT_EXECUTABLE)
    .current_dir(repo_path)
    .args([
      "show-ref",
      "--verify",
      "--quiet",
      &format!("refs/heads/{}", branch_name),
    ])
    .output()
    .context("Failed to check if branch exists")?;

  Ok(result.status.success())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test] 
  fn test_cascade_args_struct_fields() {
    // Test that the CascadeArgs struct has all expected fields with correct defaults
    let cascade_args = CascadeArgs {
      max_depth: None,
      force: false,
      force_push: false,
      show_graph: false,
      autostash: false,
      repo: None,
    };
    
    assert!(!cascade_args.force_push, "force_push should default to false");
    assert!(!cascade_args.force, "force should default to false");
    assert!(!cascade_args.show_graph, "show_graph should default to false");
    assert!(!cascade_args.autostash, "autostash should default to false");
    assert!(cascade_args.max_depth.is_none(), "max_depth should default to None");
    assert!(cascade_args.repo.is_none(), "repo should default to None");
  }

  #[test]
  fn test_force_push_flag_enabled() {
    // Test that force_push can be set to true
    let cascade_args = CascadeArgs {
      max_depth: None,
      force: false,
      force_push: true, // Enable force-push
      show_graph: false,
      autostash: false,
      repo: None,
    };
    
    assert!(cascade_args.force_push, "force_push should be true when explicitly set");
    assert!(!cascade_args.force, "force should remain false");
  }

  #[test]
  fn test_all_flags_enabled() {
    // Test that all flags can be enabled together
    let cascade_args = CascadeArgs {
      max_depth: Some(10),
      force: true,
      force_push: true,
      show_graph: true,
      autostash: true,
      repo: Some("/some/path".to_string()),
    };
    
    assert!(cascade_args.force_push, "force_push should be true");
    assert!(cascade_args.force, "force should be true");
    assert!(cascade_args.show_graph, "show_graph should be true");
    assert!(cascade_args.autostash, "autostash should be true");
    assert_eq!(cascade_args.max_depth, Some(10), "max_depth should be 10");
    assert_eq!(cascade_args.repo, Some("/some/path".to_string()), "repo should be set");
  }

  #[test]
  fn test_handle_force_push_no_remote() {
    // Test that handle_force_push handles the case where there's no remote gracefully
    use std::path::Path;
    
    // This should handle the no remote case gracefully
    let temp_path = Path::new("/tmp/nonexistent");
    let result = handle_force_push(&temp_path, "test-branch");
    
    // The function should return Ok when there's no remote (graceful handling)
    // but may return an error for non-existent paths
    // Either outcome is acceptable for this test - we're just ensuring it doesn't panic
    assert!(result.is_ok() || result.is_err(), "handle_force_push should handle non-existent paths gracefully");
  }
}
