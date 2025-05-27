use anyhow::{Context, Result};
use clap::{Arg, Command};
use colored::Colorize;
use git2::{BranchType, Repository as Git2Repository};

use crate::git::detect_current_repository;
use crate::utils::output::{format_command, print_info, print_warning};
use crate::worktree::{BranchIssue, RepoState};

/// Build the tree subcommand
pub fn build_command() -> Command {
  Command::new("tree")
    .about("Show your branch tree with associated issues and PRs")
    .long_about(
      "Display local branches in a tree-like view with their associated Jira issues and GitHub PRs.\n\n\
            This command shows all local branches in the current repository along with\n\
            any associated Jira tickets and GitHub pull requests. This gives you a\n\
            bird's-eye view of your development tree, helping you track which branches\n\
            are linked to specific issues and PRs for better workflow management.",
    )
    .alias("t")
    .arg(
      Arg::new("repo")
        .long("repo")
        .short('r')
        .help("Path to a specific repository")
        .value_name("PATH"),
    )
}

/// Handle the tree command
pub fn handle_command(tree_matches: &clap::ArgMatches) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(repo_arg) = tree_matches.get_one::<String>("repo") {
    std::path::PathBuf::from(repo_arg)
  } else {
    detect_current_repository().context("Not in a git repository")?
  };

  // Open the repository
  let repo =
    Git2Repository::open(&repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Load repository state
  let repo_state = RepoState::load(&repo_path).unwrap_or_default();

  // Get all local branches
  let branches = repo.branches(Some(BranchType::Local))?;

  // Collect branch information
  let mut branch_info = Vec::new();
  for branch_result in branches {
    let (branch, _) = branch_result?;
    if let Some(name) = branch.name()? {
      let is_current = branch.is_head();
      let branch_issue = repo_state.get_branch_issue_by_branch(name);

      branch_info.push(BranchInfo {
        name: name.to_string(),
        is_current,
        branch_issue: branch_issue.cloned(),
      });
    }
  }

  if branch_info.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  // Sort branches - current branch first, then alphabetically
  branch_info.sort_by(|a, b| {
    if a.is_current {
      std::cmp::Ordering::Less
    } else if b.is_current {
      std::cmp::Ordering::Greater
    } else {
      a.name.cmp(&b.name)
    }
  });

  // Display the tree
  display_branch_tree(&branch_info);

  Ok(())
}

#[derive(Debug)]
struct BranchInfo {
  name: String,
  is_current: bool,
  branch_issue: Option<BranchIssue>,
}

fn display_branch_tree(branches: &[BranchInfo]) {
  for (i, branch) in branches.iter().enumerate() {
    let is_last = i == branches.len() - 1;
    let tree_symbol = if is_last { "└──" } else { "├──" };

    // Build the complete line with branch, ticket, and PR info
    let mut line_parts = Vec::new();

    // Branch name with current indicator
    let branch_display = if branch.is_current {
      format!("{} {}", tree_symbol, branch.name.green().bold())
    } else {
      format!("{} {}", tree_symbol, branch.name)
    };
    line_parts.push(branch_display);

    // Add current branch indicator
    if branch.is_current {
      line_parts.push("(current)".dimmed().to_string());
    }

    // Add ticket and PR info horizontally
    if let Some(issue) = &branch.branch_issue {
      // Add Jira issue
      line_parts.push(format!("🎫 {}", issue.jira_issue.cyan()));

      // Add GitHub PR if available
      if let Some(pr_number) = issue.github_pr {
        line_parts.push(format!("🔗 PR#{}", pr_number.to_string().yellow()));
      }
    }

    // Print the complete line
    println!("{}", line_parts.join(" "));

    // Add spacing between branches (except for the last one)
    if !is_last {
      println!("│");
    }
  }

  // Show summary
  let branches_with_issues = branches.iter().filter(|b| b.branch_issue.is_some()).count();
  let branches_with_prs = branches
    .iter()
    .filter(|b| {
      b.branch_issue
        .as_ref()
        .map(|issue| issue.github_pr.is_some())
        .unwrap_or(false)
    })
    .count();

  if branches_with_issues == 0 && branches_with_prs == 0 {
    println!();
    print_info("To associate branches with issues and PRs:");
    print_info(&format!(
      "  • Link Jira issues: {}",
      format_command("twig jira branch link <issue-key>")
    ));
    print_info(&format!(
      "  • Link GitHub PRs: {}",
      format_command("twig github pr link <pr-url>")
    ));
  }
}
