//! # Rebase Command
//!
//! Derive-based implementation of the rebase command for rebasing the current
//! branch on its parent(s).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use git2::Repository as Git2Repository;
use twig_core::output::{print_error, print_info, print_success, print_warning};

use super::rebase_common::{
  ConflictResolution, RebaseContinueOutcome, RebaseResult, abort_rebase, attempt_rebase_continue, attempt_rebase_skip,
  handle_rebase_conflict, rebase_branch, rebase_branch_force, show_dependency_tree,
};
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
    twig_core::detect_repository().context("Not in a git repository")?
  };

  // Rebase the current branch on its parent(s)
  rebase_upstream(&repo_path, args.force, args.show_graph, args.autostash)
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
    let result = rebase_branch(repo_path, parent, autostash)?;

    match result {
      RebaseResult::Success => {
        print_success(&format!("Successfully rebased {current_branch_name} onto {parent}",));
      }
      RebaseResult::UpToDate => {
        if force {
          // Force rebase even if up-to-date
          print_info("Branch is up-to-date, but force flag is set. Rebasing anyway...");
          let force_result = rebase_branch_force(repo_path, parent, autostash)?;
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
        // Loop so that a second conflict arising after --continue or --skip re-prompts
        // the user rather than silently succeeding or failing.
        'conflict_loop: loop {
          print_warning(&format!(
            "Conflicts detected while rebasing {current_branch_name} onto {parent}",
          ));
          let resolution = handle_rebase_conflict(repo_path, &current_branch_name)?;

          match resolution {
            ConflictResolution::Continue => match attempt_rebase_continue(repo_path)? {
              RebaseContinueOutcome::Completed => {
                print_success(&format!(
                  "Rebase of {current_branch_name} onto {parent} completed after resolving conflicts",
                ));
                break 'conflict_loop;
              }
              RebaseContinueOutcome::MoreConflicts => continue 'conflict_loop,
              RebaseContinueOutcome::Failed => {
                print_error(&format!(
                  "Failed to continue rebase of {current_branch_name} onto {parent}. \
                   You may need to resolve conflicts manually."
                ));
                abort_rebase(repo_path)?;
                return Err(anyhow::anyhow!("Rebase failed"));
              }
            },
            ConflictResolution::AbortToOriginal | ConflictResolution::AbortStayHere => {
              abort_rebase(repo_path)?;
              print_info(&format!("Rebase of {current_branch_name} onto {parent} aborted",));
              return Ok(());
            }
            ConflictResolution::Skip => match attempt_rebase_skip(repo_path)? {
              RebaseContinueOutcome::Completed => {
                print_info(&format!(
                  "Skipped commit during rebase of {current_branch_name} onto {parent}",
                ));
                break 'conflict_loop;
              }
              RebaseContinueOutcome::MoreConflicts => continue 'conflict_loop,
              RebaseContinueOutcome::Failed => {
                print_error(&format!(
                  "Failed to skip commit during rebase of {current_branch_name} onto {parent}. \
                   You may need to resolve conflicts manually."
                ));
                abort_rebase(repo_path)?;
                return Err(anyhow::anyhow!("Rebase failed"));
              }
            },
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
