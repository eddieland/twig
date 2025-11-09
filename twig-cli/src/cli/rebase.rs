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
use twig_core::tree_renderer::TreeRenderer;

use crate::consts;
use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;
use crate::utils::is_interactive_environment;

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

  /// Fail immediately on conflicts instead of prompting interactively
  /// (useful for CI/CD environments)
  #[arg(long = "no-interactive")]
  pub no_interactive: bool,

  /// Comma-separated list of commit hashes to skip during rebase, or path to a file
  /// containing commit hashes (one per line). Commits will be excluded from the rebased branch.
  #[arg(long = "skip-commits", value_name = "COMMITS")]
  pub skip_commits: Option<String>,

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

  // Parse skip commits if provided
  let skip_commits = if let Some(ref skip_arg) = args.skip_commits {
    let commits = crate::utils::parse_skip_commits(skip_arg)?;
    
    // Validate each commit hash format
    for commit in &commits {
      if !crate::utils::validate_commit_hash(commit) {
        return Err(anyhow::anyhow!(
          "Invalid commit hash format: '{}'. Commit hashes must be 7-64 character hex strings.",
          commit
        ));
      }
    }
    
    if !commits.is_empty() {
      print_info(&format!("📋 Will skip {} commit(s) during rebase", commits.len()));
      for commit in &commits {
        print_info(&format!("  • {}", commit));
      }
    }
    
    Some(commits)
  } else {
    None
  };

  // Rebase the current branch on its parent(s)
  let force = args.force;
  let show_graph = args.show_graph;
  let autostash = args.autostash;
  // Enable non-interactive mode if explicitly requested or if not in an interactive environment
  let no_interactive = args.no_interactive || !is_interactive_environment();

  rebase_upstream(&repo_path, force, show_graph, autostash, no_interactive, skip_commits.as_deref())
}

/// Rebase current branch on its parent(s)
fn rebase_upstream(
  repo_path: &Path,
  force: bool,
  show_graph: bool,
  autostash: bool,
  no_interactive: bool,
  skip_commits: Option<&[String]>,
) -> Result<()> {
  // Check if there's already a rebase in progress
  if is_rebase_in_progress(repo_path) {
    print_warning("A rebase is already in progress.");
    print_info("You can:");
    print_info("  • Continue the rebase: git rebase --continue");
    print_info("  • Abort the rebase: git rebase --abort");
    print_info("  • Skip the current commit: git rebase --skip");
    print_info("  • Or run 'twig rebase' again after resolving the current rebase");
    return Ok(());
  }

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
    let result = rebase_branch(repo_path, &current_branch_name, parent, autostash, skip_commits)?;

    match result {
      RebaseResult::Success => {
        print_success(&format!("Successfully rebased {current_branch_name} onto {parent}",));
      }
      RebaseResult::UpToDate => {
        if force {
          // Force rebase even if up-to-date
          print_info("Branch is up-to-date, but force flag is set. Rebasing anyway...");
          let force_result = rebase_branch_force(repo_path, &current_branch_name, parent, autostash, skip_commits)?;
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
        // Check if we're in non-interactive mode
        if no_interactive {
          print_error(&format!(
            "❌ Conflicts detected while rebasing {current_branch_name} onto {parent}"
          ));
          print_error("Cannot proceed in non-interactive mode (--no-interactive).");
          print_info("To resolve conflicts manually:");
          print_info("  1. Resolve the conflicts in the working directory");
          print_info("  2. Stage the resolved files: git add <file>");
          print_info("  3. Continue the rebase: git rebase --continue");
          print_info("Or abort the rebase: git rebase --abort");
          return Err(anyhow::anyhow!(
            "Rebase conflicts detected in non-interactive mode"
          ));
        }

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
                return Ok(());
              }
            } else {
              // Rebase completed successfully after skip
              print_success(&format!(
                "Rebase of {current_branch_name} onto {parent} completed after skipping commit",
              ));
            }

            // Clean up any unmerged entries in the index and working directory after skip
            cleanup_index_after_skip(repo_path)?;

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
fn rebase_branch(
  repo_path: &Path,
  _branch: &str,
  onto: &str,
  autostash: bool,
  skip_commits: Option<&[String]>,
) -> Result<RebaseResult> {
  // If skip_commits is provided, we need to use a different approach
  if let Some(commits_to_skip) = skip_commits {
    if !commits_to_skip.is_empty() {
      return rebase_with_skip_commits(repo_path, onto, autostash, commits_to_skip);
    }
  }

  // Standard rebase without skipping commits
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

/// Rebase with specific commits skipped using interactive rebase
fn rebase_with_skip_commits(
  repo_path: &Path,
  onto: &str,
  autostash: bool,
  skip_commits: &[String],
) -> Result<RebaseResult> {
  // Get the list of commits to rebase
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(&["rev-list", "--reverse", &format!("{}..HEAD", onto)])
    .current_dir(repo_path)
    .output()
    .context("Failed to get commit list")?;

  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(anyhow::anyhow!("Failed to get commit list: {}", stderr));
  }

  let commits = String::from_utf8_lossy(&output.stdout);
  let commit_list: Vec<&str> = commits.lines().collect();

  if commit_list.is_empty() {
    return Ok(RebaseResult::UpToDate);
  }

  // Perform autostash if requested
  if autostash {
    let _ = Command::new(consts::GIT_EXECUTABLE)
      .args(&["stash", "push", "-m", "twig rebase autostash"])
      .current_dir(repo_path)
      .output();
  }

  // Reset to onto branch
  let reset_output = Command::new(consts::GIT_EXECUTABLE)
    .args(&["reset", "--hard", onto])
    .current_dir(repo_path)
    .output()
    .context("Failed to reset to target branch")?;

  if !reset_output.status.success() {
    let stderr = String::from_utf8_lossy(&reset_output.stderr);
    return Err(anyhow::anyhow!("Failed to reset to {}: {}", onto, stderr));
  }

  // Cherry-pick commits, skipping the ones in skip_commits
  let mut skipped_count = 0;
  let mut picked_count = 0;
  
  for commit_hash in &commit_list {
    let short_hash = &commit_hash[..std::cmp::min(7, commit_hash.len())];
    
    // Check if this commit should be skipped
    let should_skip = skip_commits.iter().any(|skip_hash| {
      commit_hash.starts_with(skip_hash) || skip_hash.starts_with(commit_hash)
    });

    if should_skip {
      print_info(&format!("  ⏭️  Skipping commit {}", short_hash));
      skipped_count += 1;
      continue;
    }

    // Cherry-pick the commit
    let pick_output = Command::new(consts::GIT_EXECUTABLE)
      .args(&["cherry-pick", commit_hash])
      .current_dir(repo_path)
      .output()
      .context(format!("Failed to cherry-pick commit {}", commit_hash))?;

    let stdout = String::from_utf8_lossy(&pick_output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&pick_output.stderr).to_string();

    if !pick_output.status.success() {
      // Check for conflicts
      if stderr.contains("CONFLICT") || stdout.contains("CONFLICT") {
        print_error(&format!("Conflict while cherry-picking commit {}", short_hash));
        if autostash {
          let _ = Command::new(consts::GIT_EXECUTABLE)
            .args(&["stash", "pop"])
            .current_dir(repo_path)
            .output();
        }
        return Ok(RebaseResult::Conflict);
      }
      
      print_warning(&format!("Failed to cherry-pick commit {}: {}", short_hash, stderr));
      if autostash {
        let _ = Command::new(consts::GIT_EXECUTABLE)
          .args(&["stash", "pop"])
          .current_dir(repo_path)
          .output();
      }
      return Ok(RebaseResult::Error);
    }

    picked_count += 1;
  }

  // Pop autostash if we used it
  if autostash {
    let pop_output = Command::new(consts::GIT_EXECUTABLE)
      .args(&["stash", "pop"])
      .current_dir(repo_path)
      .output();
      
    if let Ok(output) = pop_output {
      if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("No stash entries found") {
          print_warning(&format!("Failed to pop autostash: {}", stderr));
        }
      }
    }
  }

  if skipped_count > 0 {
    print_success(&format!(
      "✓ Rebase completed: {} commit(s) applied, {} commit(s) skipped",
      picked_count, skipped_count
    ));
  } else {
    print_success(&format!("✓ Rebase completed: {} commit(s) applied", picked_count));
  }

  Ok(RebaseResult::Success)
}

/// Force rebase a branch onto another branch (used with --force flag)
fn rebase_branch_force(
  repo_path: &Path,
  _branch: &str,
  onto: &str,
  autostash: bool,
  skip_commits: Option<&[String]>,
) -> Result<RebaseResult> {
  // If skip_commits is provided, use the skip-commits approach with force
  if let Some(commits_to_skip) = skip_commits {
    if !commits_to_skip.is_empty() {
      // For force rebase with skip commits, we still use the cherry-pick approach
      return rebase_with_skip_commits(repo_path, onto, autostash, commits_to_skip);
    }
  }

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

/// Check if a rebase is currently in progress
fn is_rebase_in_progress(repo_path: &Path) -> bool {
  // Check for the existence of .git/rebase-merge directory
  let rebase_merge_dir = repo_path.join(".git").join("rebase-merge");
  if rebase_merge_dir.exists() {
    return true;
  }

  // Check for the existence of .git/rebase-apply directory (used by git am and
  // some rebase operations)
  let rebase_apply_dir = repo_path.join(".git").join("rebase-apply");
  if rebase_apply_dir.exists() {
    return true;
  }

  false
}

/// Execute a git command and return the output as a string
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
