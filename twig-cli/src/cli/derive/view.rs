//! # View Command
//!
//! Derive-based implementation of the view command for displaying branches
//! with their associated issues and PRs.

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use git2::{BranchType, Repository as Git2Repository};

use crate::cli::derive::DeriveCommand;
use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::utils::output::{format_command, format_repo_path, print_header, print_info, print_warning};

/// Command for viewing branches with their associated issues and PRs
#[derive(Parser)]
#[command(name = "view")]
#[command(about = "View branches with their associated issues and PRs")]
#[command(
  long_about = "Display local branches and their associated Jira issues and GitHub PRs.\n\n\
            This command shows all local branches in the current repository along with\n\
            any associated Jira tickets and GitHub pull requests. This helps you track\n\
            which branches are linked to specific issues and PRs for better workflow management."
)]
#[command(alias = "v")]
pub struct ViewCommand {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

impl ViewCommand {
  /// Creates a clap Command for this command
  pub fn command() -> clap::Command {
    <Self as CommandFactory>::command()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute(matches: &clap::ArgMatches) -> Result<()> {
    let repo = matches.get_one::<String>("repo").cloned();

    let cmd = Self { repo };
    cmd.execute()
  }
}

impl DeriveCommand for ViewCommand {
  fn execute(self) -> Result<()> {
    // Get the repository path
    let repo_path = if let Some(repo_arg) = self.repo {
      crate::utils::resolve_repository_path(Some(&repo_arg))?
    } else {
      detect_current_repository().context("Not in a git repository")?
    };

    list_branches_with_associations(repo_path)
  }
}

/// List all local branches with their associated Jira issues and GitHub PRs
fn list_branches_with_associations<P: AsRef<std::path::Path>>(repo_path: P) -> Result<()> {
  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get all local branches
  let branches = repo
    .branches(Some(BranchType::Local))
    .context("Failed to get branches")?;

  // Load the repository state to get associations
  let state = RepoState::load(repo_path)?;

  // Collect branch information
  let mut branch_info = Vec::new();
  let mut current_branch = None;

  // Get the current branch
  if let Ok(head) = repo.head() {
    if head.is_branch() {
      current_branch = head.shorthand().map(|s| s.to_string());
    }
  }

  for branch_result in branches {
    let (branch, _) = branch_result.context("Failed to get branch")?;
    let branch_name = branch
      .name()
      .context("Failed to get branch name")?
      .unwrap_or("unknown")
      .to_string();

    // Get associated issue/PR information
    let association = state.get_branch_issue_by_branch(&branch_name);

    branch_info.push((branch_name, association));
  }

  if branch_info.is_empty() {
    print_warning("No local branches found in this repository.");
    print_info(&format!(
      "Create a branch with {}",
      format_command("git checkout -b <branch-name>")
    ));
    return Ok(());
  }

  print_header("Local Branches");
  println!("Repository: {}", format_repo_path(&repo_path.display().to_string()));

  if let Some(current) = &current_branch {
    println!("Current branch: {current}");
  }

  println!();

  // Sort branches alphabetically
  branch_info.sort_by(|a, b| a.0.cmp(&b.0));

  // Count branches and associations before iterating
  let total_branches = branch_info.len();
  let associated_branches = branch_info.iter().filter(|(_, assoc)| assoc.is_some()).count();

  // Display branch information
  for (branch_name, association) in branch_info {
    let is_current = current_branch.as_ref() == Some(&branch_name);
    let prefix = if is_current { "* " } else { "  " };

    println!("{prefix}Branch: {branch_name}");

    if let Some(assoc) = association {
      if let Some(jira_issue) = &assoc.jira_issue {
        println!("    Jira Issue: {jira_issue}");
      } else {
        println!("    Jira Issue: None");
      }
      if let Some(pr_id) = assoc.github_pr {
        println!("    GitHub PR: #{pr_id}");
      }
      println!(
        "    Associated: {}",
        crate::utils::output::format_timestamp(&assoc.created_at)
      );
    } else {
      println!("    No associations");
    }

    println!();
  }

  // Print summary

  print_info(&format!(
    "Found {total_branches} branches ({associated_branches} with associations)"
  ));

  if associated_branches < total_branches {
    print_info(&format!(
      "Link branches to issues with {}",
      format_command("twig jira branch link <issue-key>")
    ));
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use clap::CommandFactory;

  use super::*;

  #[test]
  fn verify_cli() {
    ViewCommand::command().debug_assert();
  }
}
