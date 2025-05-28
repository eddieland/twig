use anyhow::{Context, Result};
use clap::{Arg, Command};
use git2::{BranchType, Repository as Git2Repository};
use regex::Regex;

use crate::utils::output::{print_info, print_success, print_warning};
use crate::worktree::{BranchIssue, RepoState};

/// Build the sync subcommand
pub fn build_command() -> Command {
  Command::new("sync")
    .about("Automatically link branches to Jira issues and GitHub PRs")
    .long_about(
      "Scan local branches and automatically detect and link them to their corresponding\n\
            Jira issues and GitHub PRs based on branch naming conventions.\n\n\
            This command looks for patterns in branch names like:\n\
            • Jira issues: PROJ-123/feature-name, feature/PROJ-123-description\n\
            • GitHub PRs: pr-123-description, github-pr-123\n\n\
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

  print_info("Scanning branches for Jira issues and GitHub PRs...");

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
      detect_github_pr_from_branch(branch_name)
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
          let association = BranchIssue {
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
            let updated_association = BranchIssue {
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
  // 2. feature/PROJ-123-description (issue key after slash)
  // 3. feature-PROJ-123-description (issue key in middle)
  // 4. PROJ-123 (just the issue key)

  let patterns = [
    r"^([A-Z]{2,}-\d+)/", // PROJ-123/feature
    r"/([A-Z]{2,}-\d+)-", // feature/PROJ-123-desc
    r"-([A-Z]{2,}-\d+)-", // feature-PROJ-123-desc
    r"^([A-Z]{2,}-\d+)$", // just PROJ-123
    r"/([A-Z]{2,}-\d+)$", // feature/PROJ-123
  ];

  for pattern in &patterns {
    if let Ok(re) = Regex::new(pattern) {
      if let Some(captures) = re.captures(branch_name) {
        if let Some(issue_match) = captures.get(1) {
          return Some(issue_match.as_str().to_string());
        }
      }
    }
  }

  None
}

/// Detect GitHub PR number from branch name
fn detect_github_pr_from_branch(branch_name: &str) -> Option<u32> {
  // Patterns to match:
  // 1. pr-123-description
  // 2. github-pr-123
  // 3. pull-123
  // 4. pr/123

  let patterns = [
    r"^pr-(\d+)-",       // pr-123-desc
    r"^github-pr-(\d+)", // github-pr-123
    r"^pull-(\d+)",      // pull-123
    r"^pr/(\d+)",        // pr/123
  ];

  for pattern in &patterns {
    if let Ok(re) = Regex::new(pattern) {
      if let Some(captures) = re.captures(branch_name) {
        if let Some(pr_match) = captures.get(1) {
          if let Ok(pr_number) = pr_match.as_str().parse::<u32>() {
            return Some(pr_number);
          }
        }
      }
    }
  }

  None
}

/// Print summary of sync findings
fn print_sync_summary(
  detected: &[BranchIssue],
  updated: &[(BranchIssue, BranchIssue)],
  conflicts: &[(String, BranchIssue, Option<String>, Option<u32>)],
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
    print_info("  twig jira branch link <issue-key> <branch-name>");
    print_info("  twig github pr link <pr-url>");
    println!();
  }

  if detected.is_empty() && updated.is_empty() && conflicts.is_empty() {
    print_success("All branches are already properly linked!");
  }
}

/// Apply sync changes to repository state
fn apply_sync_changes(
  repo_state: &mut RepoState,
  repo_path: &std::path::Path,
  detected: Vec<BranchIssue>,
  updated: Vec<(BranchIssue, BranchIssue)>,
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

    // Test non-matching patterns
    assert_eq!(detect_jira_issue_from_branch("feature-branch"), None);
    assert_eq!(detect_jira_issue_from_branch("main"), None);
    assert_eq!(detect_jira_issue_from_branch("proj-123"), None); // lowercase
    assert_eq!(detect_jira_issue_from_branch("P-123"), None); // too short prefix
  }

  #[test]
  fn test_detect_github_pr_from_branch() {
    // Test various patterns
    assert_eq!(detect_github_pr_from_branch("pr-123-description"), Some(123));
    assert_eq!(detect_github_pr_from_branch("github-pr-456"), Some(456));
    assert_eq!(detect_github_pr_from_branch("pull-789"), Some(789));
    assert_eq!(detect_github_pr_from_branch("pr/123"), Some(123));

    // Test non-matching patterns
    assert_eq!(detect_github_pr_from_branch("feature-branch"), None);
    assert_eq!(detect_github_pr_from_branch("main"), None);
    assert_eq!(detect_github_pr_from_branch("pr-abc"), None); // non-numeric
    assert_eq!(detect_github_pr_from_branch("something-pr-123"), None); // not at start
  }
}
