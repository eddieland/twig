//! # Commit Command
//!
//! Implements the `commit` command for creating Git commits with Jira issue
//! information.

use anyhow::{Context, Result};
use clap::Args;
use tokio::runtime::Runtime;
use twig_jira::create_jira_client;

use crate::consts::{DEFAULT_JIRA_HOST, ENV_JIRA_HOST};
use crate::creds::get_jira_credentials;
use crate::git::detect_current_repository;
use crate::utils::get_current_branch_jira_issue;
use crate::utils::output::{print_error, print_info, print_success, print_warning};

/// Arguments for the commit command
#[derive(Args)]
pub struct CommitArgs {
  /// Custom message to use instead of the Jira issue summary
  #[arg(long, short = 'm')]
  pub message: Option<String>,

  /// Text to add before the issue summary (after Jira key)
  #[arg(long, short = 'p')]
  pub prefix: Option<String>,

  /// Text to add at the end of the message
  #[arg(long, short = 's')]
  pub suffix: Option<String>,

  /// Disable checking for duplicate commits to fixup
  #[arg(long)]
  pub no_fixup: bool,
}

/// Handle the commit command
pub fn handle_commit_command(args: CommitArgs) -> Result<()> {
  // Get the current repository
  let repo_path = detect_current_repository().context("Not in a git repository")?;

  // Get the current branch's Jira issue
  let jira_issue = match get_current_branch_jira_issue()? {
    Some(issue) => issue,
    None => {
      print_error("No Jira issue associated with the current branch.");
      println!("Link a Jira issue with: twig jira branch link <issue-key>");
      return Ok(());
    }
  };

  // Get Jira credentials and create client
  let credentials = get_jira_credentials().context("Failed to get Jira credentials")?;

  // Get Jira host from environment or use default
  let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

  // Create Jira client
  let jira_client = create_jira_client(&jira_host, &credentials.username, &credentials.password)?;

  // Create a runtime for async operations
  let rt = Runtime::new().context("Failed to create tokio runtime")?;

  // Fetch the issue details
  let issue = rt
    .block_on(jira_client.get_issue(&jira_issue))
    .context(format!("Failed to fetch Jira issue {jira_issue}"))?;

  // Generate the commit message using the helper function
  let commit_message = generate_commit_message(&jira_issue, &issue.fields.summary, &args);

  // Check for duplicate commit messages (unless --no-fixup is specified)
  if args.no_fixup {
    create_normal_commit(&repo_path, &commit_message)?;
  } else {
    let has_duplicate = check_for_duplicate_commit_message(&repo_path, &commit_message)?;

    if has_duplicate {
      print_warning(&format!(
        "A commit with the message '{commit_message}' already exists in recent history."
      ));

      // Ask if user wants to create a fixup commit instead
      if prompt_for_fixup()? {
        create_fixup_commit(&repo_path, &commit_message)?;
      } else {
        create_normal_commit(&repo_path, &commit_message)?;
      }
    } else {
      create_normal_commit(&repo_path, &commit_message)?;
    }
  }

  Ok(())
}

/// Check if a commit with the same message exists in recent history
fn check_for_duplicate_commit_message(repo_path: &std::path::Path, message: &str) -> Result<bool> {
  // Use git command to search recent commit messages
  let output = std::process::Command::new("git")
    .args(["log", "--pretty=format:%s", "-n", "20"]) // Check last 20 commits
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git log command")?;

  let output_str = String::from_utf8_lossy(&output.stdout);

  // Check if the message exists in recent commits
  Ok(output_str.lines().any(|line| line == message))
}

/// Prompt the user to confirm creating a fixup commit
fn prompt_for_fixup() -> Result<bool> {
  use std::io::{self, Write};

  print!("Create a fixup commit instead? [y/N]: ");
  io::stdout().flush()?;

  let mut input = String::new();
  io::stdin().read_line(&mut input)?;

  let input = input.trim().to_lowercase();
  Ok(input == "y" || input == "yes")
}

/// Create a normal commit with the given message
fn create_normal_commit(repo_path: &std::path::Path, message: &str) -> Result<()> {
  print_info(&format!("Creating commit with message: '{message}'"));

  let output = std::process::Command::new("git")
    .args(["commit", "-m", message])
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git commit command")?;

  if output.status.success() {
    print_success("Commit created successfully.");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
  } else {
    print_error("Failed to create commit.");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    Err(anyhow::anyhow!("Git commit command failed"))
  }
}

/// Create a fixup commit with the given message
fn create_fixup_commit(repo_path: &std::path::Path, message: &str) -> Result<()> {
  // Find the commit hash of the commit to fix
  let output = std::process::Command::new("git")
    .args(["log", "--pretty=format:%h %s", "-n", "20"]) // Check last 20 commits
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git log command")?;

  let output_str = String::from_utf8_lossy(&output.stdout);

  // Find the commit hash with the matching message
  let commit_hash = output_str
    .lines()
    .find_map(|line| {
      let parts: Vec<&str> = line.splitn(2, ' ').collect();
      if parts.len() == 2 && parts[1] == message {
        Some(parts[0].to_string())
      } else {
        None
      }
    })
    .context("Could not find the original commit to fix")?;

  print_info(&format!("Creating fixup commit for commit {commit_hash}"));

  let output = std::process::Command::new("git")
    .args(["commit", "--fixup", &commit_hash])
    .current_dir(repo_path)
    .output()
    .context("Failed to execute git commit --fixup command")?;

  if output.status.success() {
    print_success("Fixup commit created successfully.");
    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
  } else {
    print_error("Failed to create fixup commit.");
    println!("{}", String::from_utf8_lossy(&output.stderr));
    Err(anyhow::anyhow!("Git commit --fixup command failed"))
  }
}

/// Generate a commit message using Jira issue information
///
/// This function creates a commit message in the format "ISSUE-KEY: Summary"
/// with optional customizations:
/// - If `message` is provided, it replaces the issue summary
/// - If `prefix` is provided, it's added before the issue summary (after the
///   key)
/// - If `suffix` is provided, it's added at the end of the message
///
/// Note: When a custom message is provided, prefix and suffix are ignored.
fn generate_commit_message(issue_key: &str, issue_summary: &str, args: &CommitArgs) -> String {
  let mut commit_message = format!("{issue_key}: ");

  if let Some(message) = &args.message {
    // If custom message is provided, use it and ignore prefix/suffix
    commit_message.push_str(message);
  } else {
    // Otherwise use issue summary with optional prefix
    if let Some(prefix) = &args.prefix {
      commit_message.push_str(&format!("{prefix} "));
    }

    commit_message.push_str(issue_summary);

    // Only add suffix if we're using the issue summary
    if let Some(suffix) = &args.suffix {
      commit_message.push_str(&format!(" {suffix}"));
    }
  }

  commit_message
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_commit_message() {
    // Test with default options
    let args = CommitArgs {
      message: None,
      prefix: None,
      suffix: None,
      no_fixup: false,
    };

    let issue_key = "PROJ-123";
    let issue_summary = "Fix the bug";

    let message = generate_commit_message(issue_key, issue_summary, &args);
    assert_eq!(message, "PROJ-123: Fix the bug");

    // Test with custom message
    let args = CommitArgs {
      message: Some("Custom message".to_string()),
      prefix: None,
      suffix: None,
      no_fixup: false,
    };

    let message = generate_commit_message(issue_key, issue_summary, &args);
    assert_eq!(message, "PROJ-123: Custom message");

    // Test with prefix
    let args = CommitArgs {
      message: None,
      prefix: Some("WIP".to_string()),
      suffix: None,
      no_fixup: false,
    };

    let message = generate_commit_message(issue_key, issue_summary, &args);
    assert_eq!(message, "PROJ-123: WIP Fix the bug");

    // Test with suffix
    let args = CommitArgs {
      message: None,
      prefix: None,
      suffix: Some("[ci skip]".to_string()),
      no_fixup: false,
    };

    let message = generate_commit_message(issue_key, issue_summary, &args);
    assert_eq!(message, "PROJ-123: Fix the bug [ci skip]");

    // Test with all options
    let args = CommitArgs {
      message: Some("Override everything".to_string()),
      prefix: Some("Ignored".to_string()),
      suffix: Some("Also ignored".to_string()),
      no_fixup: false,
    };

    let message = generate_commit_message(issue_key, issue_summary, &args);
    assert_eq!(message, "PROJ-123: Override everything");
  }
}
