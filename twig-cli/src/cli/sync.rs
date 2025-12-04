//! # Sync Command
//!
//! Derive-based implementation of the sync command for automatically linking
//! branches to Jira issues and GitHub PRs.

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use clap::Parser;
use directories::BaseDirs;
use git2::{BranchType, Repository as Git2Repository};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;
use tracing::warn;
use twig_core::output::{print_info, print_success, print_warning};
use twig_core::state::{BranchMetadata, RepoState};
use twig_gh::{GitHubClient, create_github_client_from_netrc};

static JIRA_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  vec![
    Regex::new(r"^([A-Z]{2,}-\d+)(?:/|-)").unwrap(),
    Regex::new(r"/([A-Z]{2,}-\d+)-").unwrap(),
    Regex::new(r"-([A-Z]{2,}-\d+)-").unwrap(),
    Regex::new(r"^([A-Z]{2,}-\d+)$").unwrap(),
    Regex::new(r"/([A-Z]{2,}-\d+)$").unwrap(),
  ]
});

/// Command for automatically linking branches to Jira issues and GitHub PRs
#[derive(Parser)]
pub struct SyncArgs {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,

  /// Show what would be synced without making changes
  #[arg(long)]
  pub dry_run: bool,

  /// Update existing associations that differ from detected patterns
  #[arg(long)]
  pub force: bool,

  /// Skip detection and linking of Jira issues
  #[arg(long)]
  pub no_jira: bool,

  /// Skip detection and linking of GitHub PRs
  #[arg(long)]
  pub no_github: bool,
}

/// Handle the sync command
///
/// This function resolves the repository path, checks if it's in dry-run mode,
/// and then calls the `sync_branches` function to perform the actual syncing
/// of branches with their detected issues and PRs.
pub(crate) fn handle_sync_command(sync: SyncArgs) -> Result<()> {
  let repo_path = crate::utils::resolve_repository_path(sync.repo.as_deref())?;

  if sync.dry_run {
    print_info("Running in dry-run mode - no changes will be made");
  }

  sync_branches(&repo_path, sync.dry_run, sync.force, sync.no_jira, sync.no_github)
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

  let branch_names = collect_local_branch_names(&repo)?;
  let total_branches = branch_names.len();

  if total_branches == 0 {
    print_info("No local branches found to sync");
    return Ok(());
  }

  // Load current repository state
  let mut repo_state = RepoState::load(repo_path)?;

  let mut detected_associations = Vec::new();
  let mut updated_associations = Vec::new();
  let mut conflicting_associations = Vec::new();
  let mut unlinked_branches = Vec::new();

  // Create runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;
  let github_client = if !no_github {
    let base_dirs = BaseDirs::new().context("Failed to get $HOME directory")?;
    Some(create_github_client_from_netrc(base_dirs.home_dir())?)
  } else {
    None
  };

  // Create progress bar
  let pb = ProgressBar::new(total_branches as u64);
  pb.set_style(
    ProgressStyle::default_bar()
      .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {msg}")
      .unwrap()
      .progress_chars("#>-"),
  );
  pb.set_message("Scanning branches for Jira issues and GitHub PRs...");

  let repo_info = github_client
    .as_ref()
    .and_then(|gh| resolve_repo_info_from_origin(repo_path, gh));

  let jira_by_branch = detect_jira_issues_for_branches(&branch_names, no_jira);
  let github_pr_results =
    detect_github_prs_for_branches(&branch_names, github_client.as_ref(), repo_info.as_ref(), &rt, &pb);

  for (index, branch_name) in branch_names.iter().enumerate() {
    pb.set_position(index as u64);
    pb.set_message(format!("Processing: {branch_name}"));

    // Check if branch already has associations
    let existing_association = repo_state.get_branch_metadata(branch_name);

    // Detect patterns in branch name
    let detected_jira = jira_by_branch.get(branch_name).cloned().flatten();

    let detected_pr = github_pr_results
      .as_ref()
      .and_then(|results| results.get(branch_name))
      .copied()
      .flatten();

    match (detected_jira, detected_pr, existing_association) {
      // No patterns detected
      (None, None, None) => {
        unlinked_branches.push(branch_name.to_string());
      }
      // Has existing association but no patterns detected - leave as is
      (None, None, Some(_)) => {}
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
        let detected_jira = jira.clone();
        let detected_pr = pr;

        if let Some((updated_association, has_conflict)) =
          evaluate_association_update(existing, detected_jira.clone(), detected_pr)
        {
          if has_conflict && !force {
            conflicting_associations.push((branch_name.to_string(), existing.clone(), detected_jira, detected_pr));
          } else {
            updated_associations.push((existing.clone(), updated_association));
          }
        }
      }
    }
  }

  // Complete the progress bar
  pb.set_position(total_branches as u64);
  pb.set_message("Scanning complete");
  pb.finish_and_clear();

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

/// Collect names of all local branches in the repository
fn collect_local_branch_names(repo: &Git2Repository) -> Result<Vec<String>> {
  // Get all local branches
  let branches = repo
    .branches(Some(BranchType::Local))
    .context("Failed to get branches")?;

  // Collect branch names to get total count for progress bar
  Ok(
    branches
      .filter_map(|branch_result| {
        if let Ok((branch, _)) = branch_result
          && let Ok(Some(name)) = branch.name()
        {
          // Skip detached HEAD and remote tracking branches
          if name != "HEAD" && !name.contains("origin/") {
            return Some(name.to_string());
          }
        }
        None
      })
      .collect(),
  )
}

/// Detect Jira issues for a list of branch names
fn detect_jira_issues_for_branches(branch_names: &[String], no_jira: bool) -> HashMap<String, Option<String>> {
  branch_names
    .iter()
    .map(|branch| {
      let jira = if no_jira {
        None
      } else {
        detect_jira_issue_from_branch(branch)
      };
      (branch.clone(), jira)
    })
    .collect()
}

/// Detect GitHub PRs for a list of branch names
fn detect_github_prs_for_branches(
  branch_names: &[String],
  github_client: Option<&GitHubClient>,
  repo_info: Option<&(String, String)>,
  runtime: &Runtime,
  progress: &ProgressBar,
) -> Option<HashMap<String, Option<u32>>> {
  let (github_client, repo_info) = match (github_client, repo_info) {
    (Some(client), Some(info)) => (client, info),
    (Some(_), None) => {
      warn!("Skipping GitHub PR detection because the origin remote URL is missing or invalid");
      return None;
    }
    _ => return None,
  };

  progress.set_message("Fetching GitHub PRs for branches in parallel...");

  let gh_client = github_client.clone();
  let owner = repo_info.0.clone();
  let repo_name = repo_info.1.clone();
  let branch_names = branch_names.to_vec();

  let results = runtime.block_on(async move {
    let mut join_set = JoinSet::new();

    for branch_name in branch_names {
      let gh = gh_client.clone();
      let owner = owner.clone();
      let repo_name = repo_name.clone();

      join_set.spawn(async move {
        let pr = detect_github_pr_from_branch(&gh, &branch_name, &owner, &repo_name).await;
        (branch_name, pr)
      });
    }

    let mut results = HashMap::new();
    while let Some(result) = join_set.join_next().await {
      match result {
        Ok((branch_name, pr)) => {
          results.insert(branch_name, pr);
        }
        Err(error) => warn!("GitHub PR detection task failed: {error}"),
      }
    }

    results
  });

  progress.set_message("Scanning branches for Jira issues and GitHub PRs...");

  Some(results)
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
    if let Some(captures) = pattern.captures(branch_name)
      && let Some(issue_match) = captures.get(1)
    {
      return Some(issue_match.as_str().to_string());
    }
  }

  None
}

/// Resolve GitHub repository owner and name from origin remote URL
///
/// This function looks at the URL structure of the `origin` git remote to
/// extract the repository owner and name, which are then used for API calls.
///
/// NOTE: This assumes that the `origin` remote points to a GitHub repository
/// and that it is their primary / representative remote.
fn resolve_repo_info_from_origin(
  repo_path: &std::path::Path,
  github_client: &GitHubClient,
) -> Option<(String, String)> {
  let repo = Git2Repository::open(repo_path).ok()?;
  let remote = repo.find_remote("origin").ok()?;
  let remote_url = remote.url()?;

  github_client.extract_repo_info_from_url(remote_url).ok()
}

/// Detect GitHub PR number from branch using GitHub API
async fn detect_github_pr_from_branch(
  github_client: &GitHubClient,
  branch_name: &str,
  owner: &str,
  repo_name: &str,
) -> Option<u32> {
  // Search for PRs with this branch as head
  match github_client
    .find_pull_requests_by_head_branch(owner, repo_name, branch_name, None)
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

/// Determine if an existing branch association should be updated and whether
/// that update represents a conflict.
///
/// Returns an updated [`BranchMetadata`] when there is either new information
/// to fill in (e.g. missing PR number) or a conflict between detected data and
/// the stored association. The boolean indicates whether the update requires
/// user confirmation (`--force`) because it overwrites an existing value.
fn evaluate_association_update(
  existing: &BranchMetadata,
  detected_jira: Option<String>,
  detected_pr: Option<u32>,
) -> Option<(BranchMetadata, bool)> {
  if detected_jira.is_none() && detected_pr.is_none() {
    return None;
  }

  let jira_conflict = detected_jira
    .as_ref()
    .zip(existing.jira_issue.as_ref())
    .is_some_and(|(detected, existing_jira)| detected != existing_jira);

  let pr_conflict = detected_pr
    .zip(existing.github_pr)
    .is_some_and(|(detected, existing_pr)| detected != existing_pr);

  let has_new_information =
    existing.jira_issue.is_none() && detected_jira.is_some() || existing.github_pr.is_none() && detected_pr.is_some();

  let has_conflict = jira_conflict || pr_conflict;

  if !has_conflict && !has_new_information {
    return None;
  }

  let updated_association = BranchMetadata {
    branch: existing.branch.clone(),
    jira_issue: detected_jira.or_else(|| existing.jira_issue.clone()),
    github_pr: detected_pr.or(existing.github_pr),
    created_at: existing.created_at.clone(),
  };

  Some((updated_association, has_conflict))
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
      if let Some(ref jira_issue) = assoc.jira_issue
        && !jira_issue.as_str().is_empty()
      {
        parts.push(format!("Jira: {jira_issue}",));
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
  fn updates_missing_association_without_conflict() {
    let existing = BranchMetadata {
      branch: "feature/ME-123".to_string(),
      jira_issue: Some("ME-123".to_string()),
      github_pr: None,
      created_at: "timestamp".to_string(),
    };

    let (updated, has_conflict) = evaluate_association_update(&existing, None, Some(42)).expect("should update");

    assert!(!has_conflict, "missing data should not be treated as a conflict");
    assert_eq!(updated.github_pr, Some(42));
    assert_eq!(updated.jira_issue, existing.jira_issue);
  }

  #[test]
  fn flags_conflicting_detected_value() {
    let existing = BranchMetadata {
      branch: "feature/ME-123".to_string(),
      jira_issue: Some("ME-123".to_string()),
      github_pr: Some(7),
      created_at: "timestamp".to_string(),
    };

    let (updated, has_conflict) = evaluate_association_update(&existing, Some("ME-124".to_string()), Some(8))
      .expect("should update conflicting values");

    assert!(has_conflict, "overwriting existing data should be marked as conflict");
    assert_eq!(updated.jira_issue.as_deref(), Some("ME-124"));
    assert_eq!(updated.github_pr, Some(8));
  }
}
