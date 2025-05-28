//! # Sync Command
//!
//! CLI commands for synchronizing branch metadata with external services,
//! automatically detecting and linking issues from branch names and commit
//! messages.

use anyhow::{Context, Result};
use clap::{Arg, Command};
use git2::{BranchType, Repository as Git2Repository};
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::runtime::Runtime;

use crate::creds::get_github_credentials;
use crate::repo_state::{BranchMetadata, RepoState};
use crate::utils::output::{print_info, print_success, print_warning};

static JIRA_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
  vec![
    Regex::new(r"^([A-Z]{2,}-\d+)(?:/|-)").unwrap(),
    Regex::new(r"/([A-Z]{2,}-\d+)-").unwrap(),
    Regex::new(r"-([A-Z]{2,}-\d+)-").unwrap(),
    Regex::new(r"^([A-Z]{2,}-\d+)$").unwrap(),
    Regex::new(r"/([A-Z]{2,}-\d+)$").unwrap(),
  ]
});

static PR_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
  vec![
    Regex::new(r"^pr-(\d+)-").unwrap(),
    Regex::new(r"^github-pr-(\d+)").unwrap(),
    Regex::new(r"^pull-(\d+)").unwrap(),
    Regex::new(r"^pr/(\d+)").unwrap(),
  ]
});

/// Build the sync subcommand
pub fn build_command() -> Command {
  Command::new("sync")
    .about("Automatically link branches to Jira issues and GitHub PRs")
    .long_about(
      "Scan local branches and automatically detect and link them to their corresponding\n\
            Jira issues and GitHub PRs.\n\n\
            For GitHub PRs, this command:\n\
            • First searches GitHub's API for pull requests matching the branch name\n\
            • Falls back to detecting patterns in branch names if API is unavailable\n\n\
            For Jira issues, it looks for patterns in branch names like:\n\
            • PROJ-123/feature-name, feature/PROJ-123-description\n\n\
            GitHub PR branch naming patterns (fallback detection):\n\
            • pr-123-description, github-pr-123, pull-123, pr/123\n\n\
            It will automatically create associations for detected patterns and report\n\
            any branches that couldn't be linked.",
    )
    .arg(
      Arg::new("repo")
        .long("repo")
        .short('r')
        .help("Path to a specific repository")
        .value_name("PATH"),
    )
    .arg(
      Arg::new("dry-run")
        .long("dry-run")
        .help("Show what would be synced without making changes")
        .action(clap::ArgAction::SetTrue),
    )
    .arg(
      Arg::new("force")
        .long("force")
        .help("Update existing associations that differ from detected patterns")
        .action(clap::ArgAction::SetTrue),
    )
    .arg(
      Arg::new("no-jira")
        .long("no-jira")
        .help("Skip detection and linking of Jira issues")
        .action(clap::ArgAction::SetTrue),
    )
    .arg(
      Arg::new("no-github")
        .long("no-github")
        .help("Skip detection and linking of GitHub PRs")
        .action(clap::ArgAction::SetTrue),
    )
}

/// Handle the sync command
pub fn handle_command(sync_matches: &clap::ArgMatches) -> Result<()> {
  let repo_arg = sync_matches.get_one::<String>("repo").map(|s| s.as_str());
  let dry_run = sync_matches.get_flag("dry-run");
  let force = sync_matches.get_flag("force");
  let no_jira = sync_matches.get_flag("no-jira");
  let no_github = sync_matches.get_flag("no-github");

  let repo_path = crate::utils::resolve_repository_path(repo_arg)?;

  if dry_run {
    print_info("Running in dry-run mode - no changes will be made");
  }

  sync_branches(&repo_path, dry_run, force, no_jira, no_github)
}

/// Sync branches with their detected issues and PRs
fn sync_branches(
  repo_path: &std::path::Path,
  dry_run: bool,
  force: bool,
  no_jira: bool,
  no_github: bool,
) -> Result<()> {
  let repo = Git2Repository::open(repo_path)
    .with_context(|| format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get all local branches
  let branches = repo
    .branches(Some(BranchType::Local))
    .context("Failed to get branches")?;

  // Load current repository state
  let mut repo_state = RepoState::load(repo_path)?;

  let mut detected_associations = Vec::new();
  let mut updated_associations = Vec::new();
  let mut conflicting_associations = Vec::new();
  let mut unlinked_branches = Vec::new();

  println!("Scanning branches for Jira issues and GitHub PRs...");

  // Create runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  for branch_result in branches {
    let (branch, _) = branch_result.context("Failed to get branch")?;
    let branch_name = match branch.name()? {
      Some(name) => name,
      None => continue, // Skip branches without valid names
    };

    // Skip detached HEAD and remote tracking branches
    if branch_name == "HEAD" || branch_name.contains("origin/") {
      continue;
    }

    // Check if branch already has associations
    let existing_association = repo_state.get_branch_issue_by_branch(branch_name);

    // Detect patterns in branch name
    let detected_jira = if !no_jira {
      detect_jira_issue_from_branch(branch_name)
    } else {
      None
    };

    let detected_pr = if !no_github {
      // First try GitHub API detection
      let api_pr = rt.block_on(detect_github_pr_from_branch_async(branch_name, repo_path));

      // Fall back to pattern detection if API fails
      if api_pr.is_some() {
        api_pr
      } else {
        detect_github_pr_from_branch_pattern(branch_name)
      }
    } else {
      None
    };

    match (detected_jira, detected_pr, existing_association) {
      // No patterns detected
      (None, None, None) => {
        unlinked_branches.push(branch_name.to_string());
      }
      (None, None, Some(_)) => {
        // Has existing association but no patterns detected - leave as is
      }
      // New association to create
      (jira, pr, None) => {
        if jira.is_some() || pr.is_some() {
          let association = BranchMetadata {
            branch: branch_name.to_string(),
            jira_issue: jira,
            github_pr: pr,
            created_at: chrono::Utc::now().to_rfc3339(),
          };
          detected_associations.push(association);
        }
      }
      // Existing association - check for conflicts or updates
      (jira, pr, Some(existing)) => {
        let needs_update =
          (jira != existing.jira_issue && jira.is_some()) || (pr != existing.github_pr && pr.is_some());

        if needs_update {
          if force {
            let updated_association = BranchMetadata {
              branch: branch_name.to_string(),
              jira_issue: jira.or_else(|| existing.jira_issue.clone()),
              github_pr: pr.or(existing.github_pr),
              created_at: existing.created_at.clone(),
            };
            updated_associations.push((existing.clone(), updated_association));
          } else {
            conflicting_associations.push((branch_name.to_string(), existing.clone(), jira, pr));
          }
        }
      }
    }
  }

  // Report findings
  print_sync_summary(
    &detected_associations,
    &updated_associations,
    &conflicting_associations,
    &unlinked_branches,
    dry_run,
  );

  // Apply changes if not dry run
  if !dry_run {
    apply_sync_changes(&mut repo_state, repo_path, detected_associations, updated_associations)?;
  }

  Ok(())
}

/// Detect Jira issue key from branch name
fn detect_jira_issue_from_branch(branch_name: &str) -> Option<String> {
  // Patterns to match:
  // 1. PROJ-123/feature-name (issue key at start)
  // 2. PROJ-123-feature-name (issue key at start)
  // 3. feature/PROJ-123-description (issue key after slash)
  // 4. feature-PROJ-123-description (issue key in middle)
  // 5. PROJ-123 (just the issue key)
  for pattern in JIRA_PATTERNS.iter() {
    if let Some(captures) = pattern.captures(branch_name) {
      if let Some(issue_match) = captures.get(1) {
        return Some(issue_match.as_str().to_string());
      }
    }
  }

  None
}

/// Detect GitHub PR number from branch using GitHub API
async fn detect_github_pr_from_branch_async(branch_name: &str, repo_path: &std::path::Path) -> Option<u32> {
  // Get GitHub credentials
  let credentials = match get_github_credentials() {
    Ok(creds) => creds,
    Err(_) => return None, // Silently fall back if no credentials
  };

  // Create GitHub client
  let github_client = match twig_gh::create_github_client(&credentials.username, &credentials.password) {
    Ok(client) => client,
    Err(_) => return None, // Silently fall back if client creation fails
  };

  // Open the git repository to get remote info
  let repo = match Git2Repository::open(repo_path) {
    Ok(repo) => repo,
    Err(_) => return None,
  };

  let remote = match repo.find_remote("origin") {
    Ok(remote) => remote,
    Err(_) => return None,
  };

  let remote_url = match remote.url() {
    Some(url) => url,
    None => return None,
  };

  // Extract owner and repo from remote URL
  let (owner, repo_name) = match github_client.extract_repo_info_from_url(remote_url) {
    Ok((owner, repo)) => (owner, repo),
    Err(_) => return None,
  };

  // Search for PRs with this branch as head
  match github_client
    .find_pull_requests_by_head_branch(&owner, &repo_name, branch_name)
    .await
  {
    Ok(prs) => {
      // Return the first open PR, or the most recent PR if none are open
      let open_pr = prs.iter().find(|pr| pr.state == "open");
      if let Some(pr) = open_pr {
        Some(pr.number)
      } else {
        prs.first().map(|pr| pr.number)
      }
    }
    Err(_) => None, // Silently fall back if API call fails
  }
}

/// Detect GitHub PR number from branch name (fallback to pattern matching)
fn detect_github_pr_from_branch_pattern(branch_name: &str) -> Option<u32> {
  // Patterns to match:
  // 1. pr-123-description
  // 2. github-pr-123
  // 3. pull-123
  // 4. pr/123
  for pattern in PR_PATTERNS.iter() {
    if let Some(captures) = pattern.captures(branch_name) {
      if let Some(pr_match) = captures.get(1) {
        if let Ok(pr_number) = pr_match.as_str().parse::<u32>() {
          return Some(pr_number);
        }
      }
    }
  }

  None
}

/// Print summary of sync findings
fn print_sync_summary(
  detected: &[BranchMetadata],
  updated: &[(BranchMetadata, BranchMetadata)],
  conflicts: &[(String, BranchMetadata, Option<String>, Option<u32>)],
  unlinked: &[String],
  dry_run: bool,
) {
  println!();

  if !detected.is_empty() {
    let action = if dry_run { "Would create" } else { "Creating" };
    print_success(&format!("{} {} new associations:", action, detected.len()));
    for assoc in detected {
      let mut parts = Vec::new();
      if let Some(ref jira_issue) = assoc.jira_issue {
        if !jira_issue.is_empty() {
          parts.push(format!("Jira: {jira_issue}",));
        }
      }
      if let Some(pr) = assoc.github_pr {
        parts.push(format!("PR: #{pr}",));
      }
      println!("  {} -> {}", assoc.branch, parts.join(", "));
    }
    println!();
  }

  if !updated.is_empty() {
    let action = if dry_run { "Would update" } else { "Updating" };
    print_success(&format!("{} {} existing associations:", action, updated.len()));
    for (old, new) in updated {
      println!("  {}", old.branch);
      if old.jira_issue != new.jira_issue {
        println!(
          "    Jira: {} -> {}",
          old.jira_issue.as_ref().unwrap_or(&"None".to_string()),
          new.jira_issue.as_ref().unwrap_or(&"None".to_string())
        );
      }
      if old.github_pr != new.github_pr {
        println!(
          "    PR: {} -> {}",
          old.github_pr.map_or("None".to_string(), |pr| format!("#{pr}",)),
          new.github_pr.map_or("None".to_string(), |pr| format!("#{pr}",))
        );
      }
    }
    println!();
  }

  if !conflicts.is_empty() {
    print_warning(&format!("Found {} conflicting associations:", conflicts.len()));
    for (branch, existing, detected_jira, detected_pr) in conflicts {
      println!("  {branch}",);
      if detected_jira.is_some() && detected_jira != &existing.jira_issue {
        println!(
          "    Jira conflict: existing={}, detected={}",
          existing.jira_issue.as_ref().unwrap_or(&"None".to_string()),
          detected_jira.as_ref().unwrap_or(&"None".to_string())
        );
      }
      if detected_pr.is_some() && detected_pr != &existing.github_pr {
        println!(
          "    PR conflict: existing={}, detected={}",
          existing.github_pr.map_or("None".to_string(), |pr| format!("#{pr}",)),
          detected_pr.map_or("None".to_string(), |pr| format!("#{pr}",))
        );
      }
    }
    print_info("Use --force to update conflicting associations");
    println!();
  }

  if !unlinked.is_empty() {
    print_info(&format!(
      "Found {} branches without detectable patterns:",
      unlinked.len()
    ));
    for branch in unlinked {
      println!("  {branch}",);
    }
    print_info("These branches can be linked manually with:");
    println!("  twig jira branch link <issue-key> <branch-name>");
    println!("  twig github pr link <pr-url>\n");
  }

  if detected.is_empty() && updated.is_empty() && conflicts.is_empty() {
    print_success("All branches are already properly linked!");
  }
}

/// Apply sync changes to repository state
fn apply_sync_changes(
  repo_state: &mut RepoState,
  repo_path: &std::path::Path,
  detected: Vec<BranchMetadata>,
  updated: Vec<(BranchMetadata, BranchMetadata)>,
) -> Result<()> {
  let mut changes_made = false;

  // Add new associations
  for association in detected {
    repo_state.add_branch_issue(association);
    changes_made = true;
  }

  // Update existing associations
  for (_, new_association) in updated {
    repo_state.add_branch_issue(new_association);
    changes_made = true;
  }

  // Save changes if any were made
  if changes_made {
    repo_state.save(repo_path)?;
    print_success("Successfully saved branch associations");
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_detect_jira_issue_from_branch() {
    // Test various patterns
    assert_eq!(
      detect_jira_issue_from_branch("PROJ-123/feature-name"),
      Some("PROJ-123".to_string())
    );
    assert_eq!(
      detect_jira_issue_from_branch("feature/PROJ-456-description"),
      Some("PROJ-456".to_string())
    );
    assert_eq!(
      detect_jira_issue_from_branch("feature-ABC-789-description"),
      Some("ABC-789".to_string())
    );
    assert_eq!(detect_jira_issue_from_branch("PROJ-123"), Some("PROJ-123".to_string()));
    assert_eq!(
      detect_jira_issue_from_branch("feature/PROJ-123"),
      Some("PROJ-123".to_string())
    );
    assert_eq!(
      detect_jira_issue_from_branch("PROJ-123/foo"),
      Some("PROJ-123".to_string())
    );
    assert_eq!(
      detect_jira_issue_from_branch("PROJ-123-foo"),
      Some("PROJ-123".to_string())
    );

    // Test non-matching patterns
    assert_eq!(detect_jira_issue_from_branch("feature-branch"), None);
    assert_eq!(detect_jira_issue_from_branch("main"), None);
    assert_eq!(detect_jira_issue_from_branch("proj-123"), None); // lowercase
    assert_eq!(detect_jira_issue_from_branch("P-123"), None); // too short prefix
  }

  #[test]
  fn test_detect_github_pr_from_branch_pattern() {
    // Test various patterns
    assert_eq!(detect_github_pr_from_branch_pattern("pr-123-description"), Some(123));
    assert_eq!(detect_github_pr_from_branch_pattern("github-pr-456"), Some(456));
    assert_eq!(detect_github_pr_from_branch_pattern("pull-789"), Some(789));
    assert_eq!(detect_github_pr_from_branch_pattern("pr/123"), Some(123));

    // Test non-matching patterns
    assert_eq!(detect_github_pr_from_branch_pattern("feature-branch"), None);
    assert_eq!(detect_github_pr_from_branch_pattern("main"), None);
    assert_eq!(detect_github_pr_from_branch_pattern("pr-abc"), None); // non-numeric
    assert_eq!(detect_github_pr_from_branch_pattern("something-pr-123"), None); // not at start
  }
}
