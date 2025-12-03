//! # Git Operations
//!
//! Core Git functionality including repository discovery, branch operations,
//! fetching, and worktree management for the twig workflow system.

use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use git2::{BranchType, FetchOptions, Repository as Git2Repository};
use owo_colors::OwoColorize;
use serde::Serialize;
use tokio::{task, time};
use twig_core::output::{
  format_command, format_repo_name, format_repo_path, format_timestamp, print_error, print_header, print_success,
  print_warning,
};
use twig_core::{ConfigDirs, Registry, RepoState};

use crate::consts;

/// Information about a stale branch for pruning
#[derive(Debug, Clone, Serialize)]
pub struct StaleBranchInfo {
  pub name: String,
  pub last_commit_date: String,
  pub parent_branch: Option<String>,
  pub novel_commits: Vec<CommitInfo>,
  pub jira_issue: Option<String>,
  pub github_pr: Option<u32>,
}

/// Information about a commit
#[derive(Debug, Clone, Serialize)]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}

/// Summary of pruning operation
#[derive(Debug, Default)]
pub struct PruneSummary {
  pub total_stale: usize,
  pub deleted: Vec<String>,
  pub skipped: Vec<String>,
  pub errors: Vec<(String, String)>,
}

/// Add a repository to the registry
pub fn add_repository<P: AsRef<Path>>(path: P) -> Result<()> {
  let config_dirs = ConfigDirs::new()?;
  let mut registry = Registry::load(&config_dirs)?;

  let path_ref = path.as_ref();
  let canonical_path = std::fs::canonicalize(path_ref)
    .with_context(|| format!("Failed to resolve repository path {}", path_ref.display()))?;

  registry.add(&canonical_path)?;
  registry.save(&config_dirs)?;

  twig_core::state::ensure_twig_internal_gitignore(&canonical_path)?;

  Ok(())
}

/// Remove a repository from the registry
pub fn remove_repository<P: AsRef<Path>>(path: P) -> Result<()> {
  let config_dirs = ConfigDirs::new()?;
  let mut registry = Registry::load(&config_dirs)?;

  registry.remove(path)?;
  registry.save(&config_dirs)?;

  Ok(())
}

/// List all repositories in the registry
pub fn list_repositories() -> Result<()> {
  let config_dirs = ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    print_warning("No repositories in registry.");
    println!("Add one with {}", format_command("twig git add <path>"));
    return Ok(());
  }

  print_header("Tracked Repositories");
  for repo in repos {
    println!("  {} ({})", format_repo_name(&repo.name), format_repo_path(&repo.path));
  }

  Ok(())
}

/// Fetch updates for a repository
pub fn fetch_repository<P: AsRef<Path>>(path: P, all: bool) -> Result<()> {
  let path = path.as_ref();
  let repo = Git2Repository::open(path).context(format!("Failed to open git repository at {}", path.display()))?;

  let mut fetch_options = FetchOptions::new();

  if all {
    // Fetch all remotes
    let remotes = repo.remotes()?;
    for i in 0..remotes.len() {
      let remote_name = remotes.get(i).unwrap();
      println!("Fetching remote: {remote_name}");

      let mut remote = repo.find_remote(remote_name)?;
      remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .context(format!("Failed to fetch from remote '{remote_name}'"))?;
    }
  } else {
    // Just fetch origin
    println!("Fetching remote: origin");
    let mut remote = repo.find_remote("origin")?;
    remote
      .fetch(&[] as &[&str], Some(&mut fetch_options), None)
      .context("Failed to fetch from remote 'origin'")?;
  }

  // Update the last fetch time in the registry
  let config_dirs = ConfigDirs::new()?;
  let mut registry = Registry::load(&config_dirs)?;

  let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
  let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
    .unwrap()
    .to_rfc3339();

  registry
    .update_fetch_time(path, time_str)
    .context("Failed to update fetch time in registry")?;
  registry.save(&config_dirs)?;

  use twig_core::output::{format_repo_path, print_success};
  print_success(&format!(
    "Successfully fetched repository at {}",
    format_repo_path(&path.display().to_string())
  ));
  Ok(())
}

/// Fetch updates for all repositories in the registry
pub fn fetch_all_repositories() -> Result<()> {
  let config_dirs = ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    print_warning("No repositories in registry.");
    println!(
      "Add one with {}",
      twig_core::output::format_command("twig git add <path>")
    );
    return Ok(());
  }

  println!("Fetching updates for {} repositories", repos.len());

  // Create a tokio runtime for parallel execution
  let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

  rt.block_on(async {
    let mut handles = Vec::new();

    // Launch tasks for each repository
    for repo in repos {
      let repo_path = repo.path.clone();
      let repo_name = repo.name.clone();

      let handle = task::spawn(async move {
        println!(
          "Fetching repository: {} ({})",
          format_repo_name(&repo_name),
          format_repo_path(&repo_path)
        );

        let result = fetch_repository(&repo_path, true);
        (repo_name, repo_path, result)
      });

      handles.push(handle);

      // Small delay to avoid overwhelming the system
      time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all tasks to complete
    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
      match handle.await {
        Ok((_name, _path, Ok(()))) => {
          success_count += 1;
        }
        Ok((name, path, Err(e))) => {
          print_error(&format!(
            "Error fetching repository {} ({}): {}",
            format_repo_name(&name),
            format_repo_path(&path),
            e
          ));
          failure_count += 1;
        }
        Err(e) => {
          print_error(&format!("Task panicked: {e}"));
          failure_count += 1;
        }
      }
    }

    // Print summary
    println!("Fetch operation complete");
    println!("Successful: {success_count}");

    if failure_count > 0 {
      print_warning(&format!("Failed: {failure_count}"));
    }
  });

  Ok(())
}

/// Execute a command in a repository
pub fn execute_repository<P: AsRef<Path>>(path: P, command: &str) -> Result<()> {
  let path = path.as_ref();

  println!(
    "Executing in repository: {}",
    format_repo_path(&path.display().to_string())
  );

  // Split the command into program and arguments
  let mut parts = command.split_whitespace();
  let program = parts.next().unwrap_or(consts::GIT_EXECUTABLE);
  let args: Vec<&str> = parts.collect();

  // Execute the command
  let output = Command::new(program)
    .args(&args)
    .current_dir(path)
    .output()
    .context(format!("Failed to execute command: {command}"))?;

  // Print the output
  if !output.stdout.is_empty() {
    println!("{}", String::from_utf8_lossy(&output.stdout));
  }

  if !output.stderr.is_empty() {
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
  }

  if output.status.success() {
    print_success(&format!(
      "Command executed successfully in {}",
      format_repo_path(&path.display().to_string())
    ));
    Ok(())
  } else {
    print_error(&format!(
      "Command failed in {} with exit code: {}",
      format_repo_path(&path.display().to_string()),
      output.status
    ));
    Err(anyhow::anyhow!("Command execution failed"))
  }
}

/// Execute a command in all repositories
pub fn execute_all_repositories(command: &str) -> Result<()> {
  let config_dirs = ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    print_warning("No repositories in registry.");
    println!(
      "Add one with {}",
      twig_core::output::format_command("twig git add <path>")
    );
    return Ok(());
  }

  println!("Executing command in {} repositories: {}", repos.len(), command);

  // Create a tokio runtime for parallel execution
  let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

  rt.block_on(async {
    let mut handles = Vec::new();

    // Launch tasks for each repository
    for repo in repos {
      let repo_path = repo.path.clone();
      let cmd = command.to_string();

      let handle = task::spawn(async move {
        let result = execute_repository(&repo_path, &cmd);
        (repo_path, result)
      });

      handles.push(handle);

      // Small delay to avoid overwhelming the system
      time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all tasks to complete and collect results
    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
      match handle.await {
        Ok((_path, Ok(()))) => {
          success_count += 1;
        }
        Ok((_path, Err(_e))) => {
          failure_count += 1;
        }
        Err(e) => {
          print_error(&format!("Task panicked: {e}"));
          failure_count += 1;
        }
      }
    }

    // Print summary
    println!("Command execution complete");
    println!("Successful: {success_count}");

    if failure_count > 0 {
      print_warning(&format!("Failed: {failure_count}"));
    }
  });

  Ok(())
}

/// Find stale branches in a repository
pub fn find_stale_branches<P: AsRef<Path>>(path: P, days: u32, prune: bool, output_json: bool) -> Result<()> {
  let path = path.as_ref();
  let repo = Git2Repository::open(path).context(format!("Failed to open git repository at {}", path.display()))?;

  // Load repository state for user-defined dependencies
  let repo_state = RepoState::load(path)?;

  // Find stale branches using existing logic, excluding root branches
  let stale_branches = find_stale_branches_internal(&repo, &repo_state, days)?;

  if prune {
    interactive_prune_branches(path, &repo, &repo_state, stale_branches, days)
  } else {
    let mut enriched_branches = stale_branches
      .into_iter()
      .map(|branch| enhance_branch_info(&repo, &repo_state, branch))
      .collect::<Result<Vec<_>>>()?;

    // Oldest branches first for easier triage
    enriched_branches.sort_by(|a, b| a.last_commit_date.cmp(&b.last_commit_date));

    if output_json {
      display_stale_branches_json(&enriched_branches)
    } else {
      display_stale_branches(path, enriched_branches)
    }
  }
}

/// Find stale branches and return structured data
fn find_stale_branches_internal(
  repo: &Git2Repository,
  repo_state: &RepoState,
  days: u32,
) -> Result<Vec<StaleBranchInfo>> {
  // Calculate the cutoff time
  let now = SystemTime::now();
  let cutoff = now - Duration::from_secs(days as u64 * 24 * 60 * 60);
  let cutoff_secs = cutoff.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

  // Get all branches
  let branches = repo
    .branches(Some(BranchType::Local))
    .context("Failed to get branches")?;

  let mut stale_branches = Vec::new();

  for branch_result in branches {
    let (branch, _) = branch_result.context("Failed to get branch")?;
    let branch_name = branch
      .name()
      .context("Failed to get branch name")?
      .unwrap_or("unknown")
      .to_string();

    // Skip root branches - they should never be pruned
    if repo_state.is_root(&branch_name) {
      continue;
    }

    // Get the commit that the branch points to
    let commit = branch.get().peel_to_commit().context("Failed to get commit")?;
    let commit_time = commit.time().seconds();

    // Check if the branch is stale
    if commit_time < cutoff_secs {
      let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(commit_time, 0)
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

      stale_branches.push(StaleBranchInfo {
        name: branch_name,
        last_commit_date: time_str,
        parent_branch: None,
        novel_commits: vec![],
        jira_issue: None,
        github_pr: None,
      });
    }
  }

  Ok(stale_branches)
}

/// Display stale branches (non-prune mode)
fn display_stale_branches<P: AsRef<Path>>(path: P, mut stale_branches: Vec<StaleBranchInfo>) -> Result<()> {
  let path = path.as_ref();

  if stale_branches.is_empty() {
    println!(
      "No stale branches found in {}",
      format_repo_path(&path.display().to_string())
    );
  } else {
    // Ensure the list mirrors chronological order regardless of upstream sorting
    stale_branches.sort_by(|a, b| a.last_commit_date.cmp(&b.last_commit_date));

    print_warning(&format!(
      "Found {} stale branches in {}:",
      stale_branches.len(),
      format_repo_path(&path.display().to_string())
    ));

    for branch_info in stale_branches {
      let relative_time = format_relative_time(&branch_info.last_commit_date);

      println!(
        "  {} • last commit {} {}",
        branch_info.name.cyan().bold(),
        format_timestamp(&branch_info.last_commit_date).yellow(),
        format!("({relative_time})").dimmed()
      );

      if let Some(parent) = &branch_info.parent_branch {
        println!("     ↳ parent branch: {parent}");
      }

      if !branch_info.novel_commits.is_empty() {
        let total = branch_info.novel_commits.len();
        let preview = branch_info
          .novel_commits
          .iter()
          .take(3)
          .map(|commit| format!("{} {}", commit.hash.yellow(), commit.message))
          .collect::<Vec<_>>()
          .join(", ");

        if total > 3 {
          println!("     novel commits: {preview} … (showing 3 of {total})");
        } else {
          println!("     novel commits: {preview}");
        }
      }

      if let Some(jira_issue) = &branch_info.jira_issue {
        println!("     🎫 linked Jira: {jira_issue}");
      }

      if let Some(pr_number) = branch_info.github_pr {
        println!("     🔀 linked GitHub PR: #{pr_number}");
      }

      println!();
    }

    println!(
      "Run {} for interactive cleanup with detailed guidance.",
      format_command("twig git stale-branches --prune")
    );
  }

  Ok(())
}

/// Display stale branches in JSON format
fn display_stale_branches_json(stale_branches: &[StaleBranchInfo]) -> Result<()> {
  let json = serde_json::to_string_pretty(stale_branches)?;
  println!("{json}");
  Ok(())
}

/// Interactive pruning workflow
fn interactive_prune_branches<P: AsRef<Path>>(
  repo_path: P,
  repo: &Git2Repository,
  repo_state: &RepoState,
  mut stale_branches: Vec<StaleBranchInfo>,
  days: u32,
) -> Result<()> {
  if stale_branches.is_empty() {
    println!(
      "No stale branches found in {}",
      format_repo_path(&repo_path.as_ref().display().to_string())
    );
    return Ok(());
  }

  // Sort branches alphabetically
  stale_branches.sort_by(|a, b| a.name.cmp(&b.name));

  // Print initial header
  println!(
    "{}",
    format!("Finding branches not updated in the last {days} days...").dimmed()
  );
  println!(
    "{} Found {} stale branches.",
    "✔".green(),
    stale_branches.len().to_string().yellow()
  );
  println!();

  let mut summary = PruneSummary {
    total_stale: stale_branches.len(),
    ..Default::default()
  };

  for (index, branch_info) in stale_branches.into_iter().enumerate() {
    // Enhance branch info with novel commits and external data
    let enhanced_info = enhance_branch_info(repo, repo_state, branch_info)?;

    // Display branch information with progress indicator
    display_branch_for_pruning(&enhanced_info, index + 1, summary.total_stale)?;

    // Prompt user for deletion
    if prompt_for_deletion(&enhanced_info.name)? {
      match delete_branch(repo, &enhanced_info.name) {
        Ok(()) => {
          summary.deleted.push(enhanced_info.name.clone());
          println!(); // Add spacing after deletion
        }
        Err(e) => {
          summary.errors.push((enhanced_info.name.clone(), e.to_string()));
          print_error(&format!("Failed to delete {}: {}", enhanced_info.name, e));
        }
      }
    } else {
      summary.skipped.push(enhanced_info.name.clone());
    }

    println!();
  }

  display_prune_summary(&summary);
  Ok(())
}

/// Enhance branch info with novel commits and external metadata
fn enhance_branch_info(
  repo: &Git2Repository,
  repo_state: &RepoState,
  mut branch_info: StaleBranchInfo,
) -> Result<StaleBranchInfo> {
  // Find parent branch from user-defined dependencies
  let parents = repo_state.get_dependency_parents(&branch_info.name);
  branch_info.parent_branch = parents.first().map(|s| s.to_string());

  // Find novel commits if parent exists
  if let Some(parent) = &branch_info.parent_branch {
    match find_novel_commits(repo, &branch_info.name, parent) {
      Ok(commits) => {
        branch_info.novel_commits = commits;
      }
      Err(err) => {
        tracing::debug!(
          error = %err,
          branch = %branch_info.name,
          parent,
          "Failed to compute novel commits for stale branch"
        );
      }
    }
  }

  // Get Jira and GitHub metadata from repo state
  if let Some(metadata) = repo_state.get_branch_metadata(&branch_info.name) {
    branch_info.jira_issue = metadata.jira_issue.clone();
    branch_info.github_pr = metadata.github_pr;
  }

  Ok(branch_info)
}

/// Find commits in branch that are not in parent (novel commits)
fn find_novel_commits(repo: &Git2Repository, branch_name: &str, parent_name: &str) -> Result<Vec<CommitInfo>> {
  // Get branch and parent references
  let branch_ref = repo.find_branch(branch_name, git2::BranchType::Local)?;
  let parent_ref = repo.find_branch(parent_name, git2::BranchType::Local)?;

  let branch_commit = branch_ref.get().peel_to_commit()?;
  let parent_commit = parent_ref.get().peel_to_commit()?;

  // Find merge base
  let merge_base = repo.merge_base(branch_commit.id(), parent_commit.id())?;

  // Walk commits from branch to merge base
  let mut revwalk = repo.revwalk()?;
  revwalk.push(branch_commit.id())?;
  revwalk.hide(merge_base)?;

  let mut novel_commits = Vec::new();
  for commit_id in revwalk {
    let commit_id = commit_id?;
    let commit = repo.find_commit(commit_id)?;

    novel_commits.push(CommitInfo {
      hash: commit.id().to_string()[..8].to_string(),
      message: commit.message().unwrap_or("").lines().next().unwrap_or("").to_string(),
    });
  }

  Ok(novel_commits)
}

/// Calculate relative time from a timestamp string
fn format_relative_time(timestamp: &str) -> String {
  let parsed_time = chrono::DateTime::parse_from_str(&format!("{timestamp} +0000"), "%Y-%m-%d %H:%M:%S %z");

  if let Ok(time) = parsed_time {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(time.with_timezone(&chrono::Utc));

    if duration.num_days() > 0 {
      format!("{} days ago", duration.num_days())
    } else if duration.num_hours() > 0 {
      format!("{} hours ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
      format!("{} minutes ago", duration.num_minutes())
    } else {
      "just now".to_string()
    }
  } else {
    "unknown".to_string()
  }
}

/// Display branch information for pruning decision
fn display_branch_for_pruning(branch_info: &StaleBranchInfo, current: usize, total: usize) -> Result<()> {
  // Progress indicator
  let separator = "─".repeat(22);
  println!(
    "{} [{}/{}] {}",
    separator.dimmed(),
    current.to_string().dimmed(),
    total.to_string().dimmed(),
    separator.dimmed()
  );

  // Branch name with styling
  println!("🌿 Branch:      {}", branch_info.name.cyan().bold());

  // Last commit with relative time
  let relative_time = format_relative_time(&branch_info.last_commit_date);
  println!(
    "📅 Last commit: {} {}",
    branch_info.last_commit_date.yellow(),
    format!("({relative_time})").dimmed()
  );

  // Parent branch
  if let Some(parent) = &branch_info.parent_branch {
    println!("🔗 Parent:      {parent}");
  } else {
    println!("🔗 Parent:      {}", "(none)".dimmed());
  }

  // Novel commits
  if !branch_info.novel_commits.is_empty() {
    println!("📝 Commits ({}):", branch_info.novel_commits.len());
    for commit in &branch_info.novel_commits {
      println!("     {}  {}", commit.hash.yellow(), commit.message);
    }
  }

  // Display Jira info if available
  if let Some(jira_issue) = &branch_info.jira_issue {
    println!("🎫 Jira: {jira_issue}");
  }

  // Display GitHub PR info if available
  if let Some(pr_number) = branch_info.github_pr {
    println!("🔀 GitHub PR: #{pr_number}");
  }

  println!();
  Ok(())
}

/// Prompt user for deletion confirmation
fn prompt_for_deletion(branch_name: &str) -> Result<bool> {
  print!("{} [y/N]: ", format!("Delete branch '{branch_name}'?").bold());
  io::stdout().flush()?;

  let mut input = String::new();
  io::stdin().read_line(&mut input)?;

  let input = input.trim().to_lowercase();
  Ok(input == "y" || input == "yes")
}

/// Delete a git branch
fn delete_branch(repo: &Git2Repository, branch_name: &str) -> Result<()> {
  let mut branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
  branch.delete()?;
  Ok(())
}

/// Display prune summary
fn display_prune_summary(summary: &PruneSummary) {
  println!("{}", "Prune Summary".bold());
  println!("  • Total stale:  {}", summary.total_stale);

  if !summary.deleted.is_empty() {
    println!(
      "  {} {}     ({})",
      "• Deleted:".red(),
      summary.deleted.len(),
      summary.deleted.join(", ")
    );
  }

  if !summary.skipped.is_empty() {
    println!("  {} {}", "• Skipped:".yellow(), summary.skipped.len());
  }

  if !summary.errors.is_empty() {
    println!(
      "  {} {}     ({})",
      "• Errors:".red(),
      summary.errors.len(),
      summary
        .errors
        .iter()
        .map(|(branch, _)| branch.as_str())
        .collect::<Vec<_>>()
        .join(", ")
    );
  }
}

#[cfg(test)]
mod tests {
  use chrono::Utc;
  use git2::BranchType;
  use serde_json::Value;
  use twig_core::RepoState;
  use twig_test_utils::{
    GitRepoTestGuard, checkout_branch, create_branch, create_commit, create_commit_with_time, days_ago,
  };

  use super::*;

  #[test]
  fn test_find_stale_branches_with_date_filtering() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    // Create initial commit
    create_commit(repo, "README.md", "# Test Repo", "Initial commit").unwrap();

    ensure_main_branch(repo).unwrap();

    // Create a recent branch (should not be stale)
    create_branch(repo, "recent-feature", Some("main")).unwrap();
    checkout_branch(repo, "recent-feature").unwrap();
    create_commit_with_time(
      repo,
      "recent.txt",
      "recent work",
      "Recent commit",
      days_ago(5), // 5 days ago
    )
    .unwrap();

    // Create an old branch (should be stale)
    checkout_branch(repo, "main").unwrap();
    create_branch(repo, "old-feature", Some("main")).unwrap();
    checkout_branch(repo, "old-feature").unwrap();
    create_commit_with_time(
      repo,
      "old.txt",
      "old work",
      "Old commit",
      days_ago(45), // 45 days ago
    )
    .unwrap();

    // Test stale branch detection with 30-day threshold
    let repo_state = RepoState::load(git_repo.path()).unwrap();
    let stale_branches = find_stale_branches_internal(repo, &repo_state, 30).unwrap();

    assert_eq!(stale_branches.len(), 1);
    assert_eq!(stale_branches[0].name, "old-feature");
  }

  #[test]
  fn test_root_branches_excluded_from_stale_detection() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create initial commit
    create_commit(repo, "README.md", "# Test Repo", "Initial commit").unwrap();

    ensure_main_branch(repo).unwrap();

    // Create an old branch that would normally be stale
    create_branch(repo, "old-root-branch", Some("main")).unwrap();
    checkout_branch(repo, "old-root-branch").unwrap();
    create_commit_with_time(
      repo,
      "old.txt",
      "old work",
      "Old commit",
      days_ago(45), // 45 days ago - should be stale
    )
    .unwrap();

    // Create another old branch that is NOT a root
    checkout_branch(repo, "main").unwrap();
    create_branch(repo, "old-regular-branch", Some("main")).unwrap();
    checkout_branch(repo, "old-regular-branch").unwrap();
    create_commit_with_time(
      repo,
      "old2.txt",
      "old work 2",
      "Old commit 2",
      days_ago(45), // 45 days ago - should be stale
    )
    .unwrap();

    // Set up repository state and mark one branch as root
    let mut repo_state = RepoState::load(repo_path).unwrap();
    repo_state.add_root("old-root-branch".to_string(), false).unwrap();
    repo_state.save(repo_path).unwrap();

    // Test stale branch detection with 30-day threshold
    let stale_branches = find_stale_branches_internal(repo, &repo_state, 30).unwrap();

    // Should only find the regular old branch, not the root branch
    assert_eq!(stale_branches.len(), 1);
    assert_eq!(stale_branches[0].name, "old-regular-branch");

    // Verify the root branch is not in the results
    assert!(!stale_branches.iter().any(|b| b.name == "old-root-branch"));
  }

  #[test]
  fn test_find_novel_commits() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    // Create initial commit and main branch
    create_commit(repo, "README.md", "# Test Repo", "Initial commit").unwrap();
    ensure_main_branch(repo).unwrap();

    // Add some commits to main
    create_commit(repo, "main1.txt", "main work 1", "Main commit 1").unwrap();
    create_commit(repo, "main2.txt", "main work 2", "Main commit 2").unwrap();

    // Create feature branch from main
    create_branch(repo, "feature", Some("main")).unwrap();
    checkout_branch(repo, "feature").unwrap();

    // Add novel commits to feature branch
    create_commit(repo, "feature1.txt", "feature work 1", "Feature commit 1").unwrap();
    create_commit(repo, "feature2.txt", "feature work 2", "Feature commit 2").unwrap();

    // Test novel commit detection
    let novel_commits = find_novel_commits(repo, "feature", "main").unwrap();

    assert_eq!(novel_commits.len(), 2);
    assert!(novel_commits.iter().any(|c| c.message == "Feature commit 1"));
    assert!(novel_commits.iter().any(|c| c.message == "Feature commit 2"));
  }

  #[test]
  fn test_enhance_branch_info_with_dependencies() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;
    let repo_path = git_repo.path();

    // Create test branches
    create_commit(repo, "README.md", "# Test Repo", "Initial commit").unwrap();
    ensure_main_branch(repo).unwrap();
    create_branch(repo, "feature", Some("main")).unwrap();

    // Set up repository state with dependencies
    let mut repo_state = RepoState::load(repo_path).unwrap();
    repo_state
      .add_dependency("feature".to_string(), "main".to_string())
      .unwrap();
    repo_state.save(repo_path).unwrap();

    // Create branch info
    let branch_info = StaleBranchInfo {
      name: "feature".to_string(),
      last_commit_date: "2024-01-01".to_string(),
      parent_branch: None,
      novel_commits: vec![],
      jira_issue: None,
      github_pr: None,
    };

    // Test enhancement
    let enhanced = enhance_branch_info(repo, &repo_state, branch_info).unwrap();

    assert_eq!(enhanced.parent_branch, Some("main".to_string()));
  }

  #[test]
  fn test_branch_deletion() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    // Create test branch
    create_commit(repo, "README.md", "# Test Repo", "Initial commit").unwrap();
    create_branch(repo, "test-branch", None).unwrap();

    // Verify branch exists
    assert!(repo.find_branch("test-branch", git2::BranchType::Local).is_ok());

    // Delete branch
    delete_branch(repo, "test-branch").unwrap();

    // Verify branch is deleted
    assert!(repo.find_branch("test-branch", git2::BranchType::Local).is_err());
  }

  #[test]
  fn test_display_stale_branches_empty() {
    let git_repo = GitRepoTestGuard::new();
    let repo_path = git_repo.path();

    let stale_branches = vec![];
    let result = display_stale_branches(repo_path, stale_branches);
    assert!(result.is_ok());
  }

  #[test]
  fn test_display_stale_branches_with_data() {
    let git_repo = GitRepoTestGuard::new();
    let repo_path = git_repo.path();

    let stale_branches = vec![StaleBranchInfo {
      name: "old-branch".to_string(),
      last_commit_date: "2024-01-01 12:00:00".to_string(),
      parent_branch: None,
      novel_commits: vec![],
      jira_issue: None,
      github_pr: None,
    }];

    let result = display_stale_branches(repo_path, stale_branches);
    assert!(result.is_ok());
  }

  #[test]
  fn test_stale_branch_info_serializes_to_json() {
    let info = StaleBranchInfo {
      name: "feature/test".into(),
      last_commit_date: "2024-01-01 12:00:00".into(),
      parent_branch: Some("main".into()),
      novel_commits: vec![CommitInfo {
        hash: "abcdef12".into(),
        message: "Test commit".into(),
      }],
      jira_issue: Some("PROJ-123".into()),
      github_pr: Some(42),
    };

    let json: Value = serde_json::to_value(vec![info]).unwrap();

    assert_eq!(json[0]["name"], "feature/test");
    assert_eq!(json[0]["parent_branch"], "main");
    assert_eq!(json[0]["jira_issue"], "PROJ-123");
    assert_eq!(json[0]["github_pr"], 42);
    assert_eq!(json[0]["novel_commits"][0]["hash"], "abcdef12");
  }

  #[test]
  fn test_enhance_branch_info_missing_parent_branch_does_not_fail() {
    let git_repo = GitRepoTestGuard::new();
    let repo = &git_repo.repo;

    create_commit(repo, "README.md", "# Test Repo", "Initial commit").unwrap();
    ensure_main_branch(repo).unwrap();

    create_branch(repo, "feature/child", Some("main")).unwrap();

    let mut repo_state = RepoState::default();
    repo_state
      .add_dependency("feature/child".into(), "feature/missing-parent".into())
      .unwrap();

    let info = StaleBranchInfo {
      name: "feature/child".into(),
      last_commit_date: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
      parent_branch: None,
      novel_commits: vec![],
      jira_issue: None,
      github_pr: None,
    };

    let enhanced = enhance_branch_info(repo, &repo_state, info).unwrap();

    assert_eq!(enhanced.parent_branch.as_deref(), Some("feature/missing-parent"));
    assert!(enhanced.novel_commits.is_empty());
  }

  fn ensure_main_branch(repo: &git2::Repository) -> anyhow::Result<()> {
    if repo.find_branch("main", BranchType::Local).is_ok() {
      return checkout_branch(repo, "main");
    }

    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("main", &head_commit, false)?;
    checkout_branch(repo, "main")
  }
}
