//! # Rebase Command
//!
//! Derive-based implementation of the rebase command for rebasing the current
//! branch on its parent(s).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;
use git2::Repository as Git2Repository;
use twig_core::detect_repository;
use twig_core::output::{print_error, print_info, print_success, print_warning};

use crate::consts;
use crate::tree_renderer::TreeRenderer;
use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;

/// Command for rebasing the current branch on its parent(s)
#[derive(Args)]
pub struct RebaseArgs {
  /// Force rebase even if branches are up-to-date
  #[arg(long)]
  pub force: bool,

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

/// Handle the rebase command
pub fn handle_rebase_command(args: RebaseArgs) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(ref repo_arg) = args.repo {
    PathBuf::from(repo_arg)
  } else {
    detect_repository().context("Not in a git repository")?
  };

  // Rebase the current branch on its parent(s)
  let force = args.force;
  let show_graph = args.show_graph;
  let autostash = args.autostash;

  rebase_upstream(&repo_path, force, show_graph, autostash)
}

/// Rebase current branch on its parent(s)
fn rebase_upstream(repo_path: &Path, force: bool, show_graph: bool, autostash: bool) -> Result<()> {
  // Open the repository
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the current branch
  let head = repo.head()?;
  if !head.is_branch() {
    return Err(anyhow::anyhow!("HEAD is not a branch. Cannot rebase."));
  }

  let current_branch_name = head.shorthand().unwrap_or("HEAD").to_string();
  print_info(&format!("Current branch: {current_branch_name}",));

  // Load repository state
  let repo_state = twig_core::state::RepoState::load(repo_path).unwrap_or_default();

  // Create the user-defined dependency resolver
  let resolver = UserDefinedDependencyResolver;

  // Build the branch node tree structure
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;

  // Check if we have any branches at all
  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  // Get the parents of the current branch
  let parents = repo_state.get_dependency_parents(&current_branch_name);

  if parents.is_empty() {
    print_warning("No parent branches found for the current branch.");
    print_info("Use 'twig branch depend <parent-branch>' to define a parent branch.");
    return Ok(());
  }

  // Show dependency graph if requested
  if show_graph {
    show_dependency_tree(repo_path, &current_branch_name)?;
  }

  // Rebase on each parent
  for parent in parents {
    print_info(&format!("Rebasing {current_branch_name} onto {parent}",));

    // Execute the rebase
    let result = rebase_branch(repo_path, &current_branch_name, parent, autostash)?;

    match result {
      RebaseResult::Success => {
        print_success(&format!("Successfully rebased {current_branch_name} onto {parent}",));
      }
      RebaseResult::UpToDate => {
        if force {
          // Force rebase even if up-to-date
          print_info("Branch is up-to-date, but force flag is set. Rebasing anyway...");
          let force_result = rebase_branch_force(repo_path, &current_branch_name, parent, autostash)?;
          match force_result {
            RebaseResult::Success => {
              print_success(&format!(
                "Successfully force-rebased {current_branch_name} onto {parent}"
              ));
            }
            _ => {
              print_error(&format!("Failed to force-rebase {current_branch_name} onto {parent}"));
              return Err(anyhow::anyhow!("Rebase failed"));
            }
          }
        } else {
          print_info(&format!(
            "Branch {current_branch_name} is already up-to-date with {parent}"
          ));
        }
      }
      RebaseResult::Conflict => {
        // Handle conflict
        print_warning(&format!(
          "Conflicts detected while rebasing {current_branch_name} onto {parent}",
        ));
        let resolution = handle_rebase_conflict(repo_path, &current_branch_name)?;

        match resolution {
          ConflictResolution::Continue => {
            // Continue the rebase
            let continue_result = execute_git_command(repo_path, &["rebase", "--continue"])?;
            print_info(&continue_result);
            print_success(&format!(
              "Rebase of {current_branch_name} onto {parent} completed after resolving conflicts",
            ));
          }
          ConflictResolution::AbortToOriginal => {
            // Abort the rebase and go back to the original branch
            let abort_result = execute_git_command(repo_path, &["rebase", "--abort"])?;
            print_info(&abort_result);
            print_info(&format!("Rebase of {current_branch_name} onto {parent} aborted",));
            return Ok(());
          }
          ConflictResolution::AbortStayHere => {
            // Abort the rebase but stay on the current branch
            let abort_result = execute_git_command(repo_path, &["rebase", "--abort"])?;
            print_info(&abort_result);
            print_info(&format!("Rebase of {current_branch_name} onto {parent} aborted",));
            return Ok(());
          }
          ConflictResolution::Skip => {
            // Skip the current commit
            let skip_result = execute_git_command(repo_path, &["rebase", "--skip"])?;
            print_info(&skip_result);
            print_info(&format!(
              "Skipped commit during rebase of {current_branch_name} onto {parent}",
            ));
          }
        }
      }
      RebaseResult::Error => {
        print_error(&format!("Failed to rebase {current_branch_name} onto {parent}",));
        return Err(anyhow::anyhow!("Rebase failed"));
      }
    }
  }

  Ok(())
}

/// Show the dependency tree
fn show_dependency_tree(repo_path: &Path, _current_branch: &str) -> Result<()> {
  // Open the repository
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Load repository state
  let repo_state = twig_core::state::RepoState::load(repo_path).unwrap_or_default();

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
