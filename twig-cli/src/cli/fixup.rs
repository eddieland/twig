//! # Fixup Command
//!
//! Implements the `fixup` command for creating fixup commits with an
//! interactive commit selector using skim fuzzy finder.

use anyhow::{Context, Result};
use clap::Args;
use twig_core::detect_repository;
use twig_core::output::{print_info, print_success, print_warning};

use crate::fixup::{commit_collector, creator, scorer, selector};

/// Arguments for the fixup command
#[derive(Args)]
pub struct FixupArgs {
  /// Number of recent commits to consider
  #[arg(long, default_value = "20")]
  pub limit: usize,

  /// Only consider commits from the last N days
  #[arg(long, default_value = "30")]
  pub days: u32,

  /// Include commits from all authors, not just current user
  #[arg(long)]
  pub all_authors: bool,

  /// Show what would be done without creating the fixup commit
  #[arg(long)]
  pub dry_run: bool,
}

/// Handle the fixup command
pub fn handle_fixup_command(args: FixupArgs) -> Result<()> {
  // Get the current repository
  let repo_path = detect_repository().context("Not in a git repository")?;

  // Check if there are staged changes before proceeding
  if !creator::has_staged_changes(&repo_path)? {
    print_warning("No staged changes found. Stage changes first before creating a fixup commit.");
    return Ok(());
  }

  tracing::info!("Analyzing recent commits on current branch...");

  // Collect commit candidates
  let mut candidates = commit_collector::collect_commits(&repo_path, &args)?;

  if candidates.is_empty() {
    print_warning("No recent commits found. Try increasing --limit or --days.");
    return Ok(());
  }

  // Get current branch Jira issue for scoring
  let current_jira_issue = twig_core::get_current_branch_jira_issue().unwrap_or(None);

  // Score and sort candidates
  scorer::score_commits(&mut candidates, &args, current_jira_issue)?;

  tracing::debug!("Found {} commit candidates", candidates.len());

  // Launch interactive selector
  let selected_commit = match selector::select_commit(&candidates)? {
    Some(commit) => commit,
    None => {
      // User cancelled selection - exit silently
      return Ok(());
    }
  };

  tracing::info!("Creating fixup commit for {}...", selected_commit.short_hash);

  // Create the fixup commit
  if args.dry_run {
    print_info(&format!(
      "Would create fixup commit for: {} {}",
      selected_commit.short_hash, selected_commit.message
    ));
  } else {
    creator::create_fixup_commit(&repo_path, &selected_commit)?;
    print_success("Fixup commit created successfully.");
  }

  Ok(())
}
