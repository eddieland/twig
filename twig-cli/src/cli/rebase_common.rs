//! Shared utilities for rebase operations.
//!
//! Contains types and helpers used by both the standalone `rebase` command and
//! the cascading `cascade` command to keep conflict-resolution behaviour
//! consistent.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use git2::Repository as Git2Repository;
use twig_core::output::{print_info, print_warning};
use twig_core::tree_renderer::TreeRenderer;
use twig_core::twig_theme;

use crate::consts;
use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;

/// Output from a git command, including both the combined stdout/stderr text and
/// whether the process exited successfully (exit code 0).
pub struct GitCommandOutput {
  /// Combined stdout and stderr text.
  pub output: String,
  /// Whether the command exited with status code 0.
  pub success: bool,
}

/// Result of an initial `git rebase` invocation.
pub enum RebaseResult {
  Success,
  UpToDate,
  Conflict,
  Error,
}

/// User-chosen action when a rebase conflict is encountered.
pub enum ConflictResolution {
  Continue,
  AbortToOriginal,
  AbortStayHere,
  Skip,
}

/// Outcome of a `git rebase --continue` or `git rebase --skip` attempt.
pub enum RebaseContinueOutcome {
  /// The rebase completed successfully.
  Completed,
  /// Another conflict was encountered; the caller should re-prompt.
  MoreConflicts,
  /// The command failed (non-zero exit) without a new conflict.
  Failed,
}

/// Execute a git command and return its output along with the exit status.
pub fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<GitCommandOutput> {
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(args)
    .current_dir(repo_path)
    .output()
    .context(format!("Failed to execute git command: {args:?}"))?;

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

/// Rebase the currently checked-out branch onto `onto`.
pub fn rebase_branch(repo_path: &Path, onto: &str, autostash: bool) -> Result<RebaseResult> {
  run_rebase(repo_path, onto, autostash, false)
}

/// Force-rebase the currently checked-out branch onto `onto`.
pub fn rebase_branch_force(repo_path: &Path, onto: &str, autostash: bool) -> Result<RebaseResult> {
  run_rebase(repo_path, onto, autostash, true)
}

/// Shared implementation for both normal and force rebase.
fn run_rebase(repo_path: &Path, onto: &str, autostash: bool, force: bool) -> Result<RebaseResult> {
  let mut args = vec!["rebase"];
  if force {
    args.push("--force-rebase");
  }
  if autostash {
    args.push("--autostash");
  }
  args.push(onto);

  let result = execute_git_command(repo_path, &args)?;

  if !result.output.is_empty() {
    print_info(&result.output);
  }

  if result.output.contains("up to date") {
    return Ok(RebaseResult::UpToDate);
  }
  if result.output.contains("CONFLICT") {
    return Ok(RebaseResult::Conflict);
  }
  if result.success {
    Ok(RebaseResult::Success)
  } else {
    Ok(RebaseResult::Error)
  }
}

/// Prompt the user to choose how to resolve a rebase conflict.
pub fn handle_rebase_conflict() -> Result<ConflictResolution> {
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

/// Attempt `git rebase --continue` and classify the outcome.
///
/// Checks for new conflicts first (so the caller can re-prompt), then checks
/// the exit status so that hard failures are never silently treated as success.
pub fn attempt_rebase_continue(repo_path: &Path) -> Result<RebaseContinueOutcome> {
  attempt_rebase_action(repo_path, "--continue")
}

/// Attempt `git rebase --skip` and classify the outcome.
///
/// Checks for new conflicts first (so the caller can re-prompt), then checks
/// the exit status so that hard failures are never silently treated as success.
pub fn attempt_rebase_skip(repo_path: &Path) -> Result<RebaseContinueOutcome> {
  attempt_rebase_action(repo_path, "--skip")
}

/// Abort an in-progress rebase, printing any output.
pub fn abort_rebase(repo_path: &Path) -> Result<()> {
  let result = execute_git_command(repo_path, &["rebase", "--abort"])?;
  if !result.output.is_empty() {
    print_info(&result.output);
  }
  Ok(())
}

/// Shared implementation for `--continue` and `--skip` rebase actions.
fn attempt_rebase_action(repo_path: &Path, action: &str) -> Result<RebaseContinueOutcome> {
  let result = execute_git_command(repo_path, &["rebase", action])?;
  if !result.output.is_empty() {
    print_info(&result.output);
  }
  if result.output.contains("CONFLICT") {
    return Ok(RebaseContinueOutcome::MoreConflicts);
  }
  if !result.success {
    return Ok(RebaseContinueOutcome::Failed);
  }
  Ok(RebaseContinueOutcome::Completed)
}

/// Print the branch dependency tree for the repository.
pub fn show_dependency_tree(repo_path: &Path, _current_branch: &str) -> Result<()> {
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  let repo_state = twig_core::state::RepoState::load(repo_path).unwrap_or_default();
  let resolver = UserDefinedDependencyResolver;
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;
  let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

  if roots.is_empty() {
    print_warning("No root branches found. Cannot display dependency tree.");
    return Ok(());
  }

  print_info("Branch dependency tree:");

  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, None, false);
  let mut stdout = std::io::stdout();
  for (i, root) in roots.iter().enumerate() {
    if i > 0 {
      println!();
    }
    renderer.render_tree(&mut stdout, root, 0, &[], false)?;
  }

  if !orphaned.is_empty() {
    println!("\n📝 Orphaned branches (no dependencies defined):");
    for branch in orphaned {
      println!("  • {branch}");
    }
  }

  Ok(())
}
