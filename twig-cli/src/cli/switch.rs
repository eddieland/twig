//! # Switch Command
//!
//! Derive-based implementation of the switch command for intelligently
//! switching to branches based on various inputs.

use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use directories::BaseDirs;
use git2::Repository as Git2Repository;
use tokio::runtime::Runtime;
use twig_core::detect_repository;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::state::{BranchMetadata, RepoState};
use twig_gh::GitHubClient;
use twig_jira::JiraClient;

use crate::clients::{self, get_jira_host};

/// Command for intelligently switching to branches based on various inputs
#[derive(Args)]
pub struct SwitchArgs {
  #[arg(
    required = true,
    index = 1,
    long_help = "Jira issue, GitHub PR, or branch name\n\n\
               Can be any of the following:\n\
               • Jira issue key (PROJ-123)\n\
               • Jira issue URL (https://company.atlassian.net/browse/PROJ-123)\n\
               • GitHub PR ID (12345 or PR#12345)\n\
               • GitHub PR URL (https://github.com/owner/repo/pull/123)\n\
               • Branch name (feature/my-branch)"
  )]
  pub input: String,

  #[arg(
    long = "no-create",
    long_help = "Don't create branch if it doesn't exist\n\n\
               Disable the default behavior of creating branches when they don't exist.\n\
               By default, twig switch will create missing branches. Use this flag\n\
               to only switch to existing branches."
  )]
  pub no_create: bool,

  #[arg(
    short,
    long,
    value_name = "PARENT",
    num_args = 0..=1,
    default_missing_value = "current",
    long_help = "Set parent dependency for the new branch\n\n\
               Specify a parent branch to create a dependency relationship.\n\
               Values can be:\n\
               • 'current' (default if flag used without value): Use current branch\n\
               • A branch name: Use the specified branch\n\
               • A Jira issue key (e.g., PROJ-123): Use branch associated with Jira issue\n\
               • 'none': Don't set any parent (use default root)"
  )]
  pub parent: Option<String>,
}

/// Handle the switch command
///
/// This function detects the type of input provided (Jira issue, GitHub PR, or
/// branch name) and switches to the appropriate branch.
pub(crate) fn handle_switch_command(switch: SwitchArgs) -> Result<()> {
  let input = &switch.input;
  let create_if_missing = !switch.no_create;
  let parent_option = switch.parent.as_deref();

  // Get the current repository
  let repo_path = detect_repository().context("Not in a git repository")?;

  // Detect input type and handle accordingly
  match detect_input_type(input) {
    InputType::JiraIssueKey(issue_key) | InputType::JiraIssueUrl(issue_key) => {
      let jira_host = get_jira_host().context("Failed to get Jira host")?;

      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let jira = clients::create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host)
        .context("Failed to create Jira client")?;

      handle_jira_switch(&jira, &repo_path, &issue_key, create_if_missing, parent_option)
    }
    InputType::GitHubPrId(pr_number) | InputType::GitHubPrUrl(pr_number) => {
      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let gh =
        clients::create_github_client_from_netrc(base_dirs.home_dir()).context("Failed to create GitHub client")?;

      handle_github_pr_switch(&gh, &repo_path, pr_number, create_if_missing, parent_option)
    }
    InputType::BranchName(branch_name) => {
      handle_branch_switch(&repo_path, &branch_name, create_if_missing, parent_option)
    }
  }
}

/// Input type detection
#[derive(Debug)]
enum InputType {
  JiraIssueKey(String),
  JiraIssueUrl(String),
  GitHubPrId(u32),
  GitHubPrUrl(u32),
  BranchName(String),
}

/// Detect the type of input provided
fn detect_input_type(input: &str) -> InputType {
  // Check for GitHub PR URL
  if input.contains("github.com") && input.contains("/pull/") {
    if let Ok(pr_number) = extract_pr_number_from_url(input) {
      return InputType::GitHubPrUrl(pr_number);
    }
  }

  // Check for Jira issue URL
  if input.contains("atlassian.net/browse/") || (input.starts_with("http") && input.contains("/browse/")) {
    if let Some(issue_key) = extract_jira_issue_from_url(input) {
      return InputType::JiraIssueUrl(issue_key);
    }
  }

  // Check for GitHub PR ID patterns (123, PR#123, #123)
  let cleaned_input = input.trim_start_matches("PR#").trim_start_matches('#');
  if let Ok(pr_number) = cleaned_input.parse::<u32>() {
    return InputType::GitHubPrId(pr_number);
  }

  // Check for Jira issue key pattern (PROJ-123, ABC-456, etc.)
  if is_jira_issue_key(input) {
    return InputType::JiraIssueKey(input.to_string());
  }

  // Default to branch name
  InputType::BranchName(input.to_string())
}

/// Check if input matches Jira issue key pattern
fn is_jira_issue_key(input: &str) -> bool {
  // Jira issue keys typically follow the pattern: PROJECT-123
  // Where PROJECT is 2+ uppercase letters, followed by hyphen and number
  let re = regex::Regex::new(r"^[A-Z]{2,}-\d+$").unwrap();
  re.is_match(input)
}

/// Extract PR number from GitHub URL
fn extract_pr_number_from_url(url: &str) -> Result<u32> {
  let re = regex::Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").context("Failed to compile regex")?;

  if let Some(captures) = re.captures(url) {
    let pr_str = captures.get(1).unwrap().as_str();
    let pr_number = pr_str
      .parse::<u32>()
      .with_context(|| format!("Failed to parse PR number '{pr_str}' as a valid integer"))?;
    Ok(pr_number)
  } else {
    Err(anyhow::anyhow!("Could not extract PR number from URL: {}", url))
  }
}

/// Extract Jira issue key from Jira URL
fn extract_jira_issue_from_url(url: &str) -> Option<String> {
  let re = regex::Regex::new(r"/browse/([A-Z]{2,}-\d+)").ok()?;
  re.captures(url)
    .and_then(|captures| captures.get(1))
    .map(|m| m.as_str().to_string())
}

/// Resolve parent branch based on the provided option
fn resolve_parent_branch(repo_path: &std::path::Path, parent_option: Option<&str>) -> Result<Option<String>> {
  match parent_option {
    None => Ok(None), // No parent specified
    Some("current") => {
      // Get the current branch name
      let repo = Git2Repository::open(repo_path)?;
      let head = repo.head()?;
      if head.is_branch() {
        let branch_name = head.shorthand().unwrap_or_default().to_string();
        Ok(Some(branch_name))
      } else {
        print_warning("HEAD is not on a branch, cannot use as parent");
        Ok(None)
      }
    }
    Some("none") => Ok(None), // Explicitly no parent
    Some(parent) => {
      // Check if it's a Jira issue key
      if is_jira_issue_key(parent) {
        // Look up branch by Jira issue
        let repo_state = RepoState::load(repo_path)?;
        if let Some(branch_issue) = repo_state.get_branch_issue_by_jira(parent) {
          Ok(Some(branch_issue.branch.clone()))
        } else {
          print_warning(&format!("No branch found for Jira issue {parent}"));
          Ok(None)
        }
      } else {
        // Assume it's a branch name, verify it exists
        let repo = Git2Repository::open(repo_path)?;
        if repo.find_branch(parent, git2::BranchType::Local).is_ok() {
          Ok(Some(parent.to_string()))
        } else {
          print_warning(&format!("Branch '{parent}' not found, cannot use as parent"));
          Ok(None)
        }
      }
    }
  }
}

/// Handle switching to a branch based on Jira issue
fn handle_jira_switch(
  jira: &JiraClient,
  repo_path: &Path,
  issue_key: &str,
  create_if_missing: bool,
  parent_option: Option<&str>,
) -> Result<()> {
  print_info(&format!("Looking for branch associated with Jira issue: {issue_key}",));

  // Load repository state to find associated branch
  let repo_state = RepoState::load(repo_path)?;

  // Look for existing branch association
  if let Some(branch_issue) = repo_state.get_branch_issue_by_jira(issue_key) {
    let branch_name = &branch_issue.branch;
    print_info(&format!("Found associated branch: {branch_name}",));
    return switch_to_branch(repo_path, branch_name);
  }

  // No existing association found
  if create_if_missing {
    print_info("No associated branch found. Creating new branch from Jira issue...");
    create_branch_from_jira_issue(jira, repo_path, issue_key, parent_option)
  } else {
    print_warning(&format!(
      "No branch found for Jira issue {issue_key}. Use --create to create a new branch.",
    ));
    Ok(())
  }
}

/// Handle switching to a branch based on GitHub PR
fn handle_github_pr_switch(
  gh: &GitHubClient,
  repo_path: &Path,
  pr_number: u32,
  create_if_missing: bool,
  parent_option: Option<&str>,
) -> Result<()> {
  print_info(&format!("Looking for branch associated with GitHub PR: #{pr_number}",));

  // Load repository state to find associated branch
  let repo_state = RepoState::load(repo_path)?;

  // Look for existing branch association
  for branch_issue in repo_state.list_branch_issues() {
    if let Some(github_pr) = branch_issue.github_pr {
      if github_pr == pr_number {
        let branch_name = &branch_issue.branch;
        print_info(&format!("Found associated branch: {branch_name}",));
        return switch_to_branch(repo_path, branch_name);
      }
    }
  }

  // No existing association found
  if create_if_missing {
    print_info("No associated branch found. Creating new branch from GitHub PR...");
    create_branch_from_github_pr(gh, repo_path, pr_number, parent_option)
  } else {
    print_warning(&format!(
      "No branch found for GitHub PR #{pr_number}. Use --create to create a new branch.",
    ));
    Ok(())
  }
}

/// Handle switching to a branch by name
fn handle_branch_switch(
  repo_path: &std::path::Path,
  branch_name: &str,
  create_if_missing: bool,
  parent_option: Option<&str>,
) -> Result<()> {
  let repo = Git2Repository::open(repo_path)?;

  // Check if branch exists
  if repo.find_branch(branch_name, git2::BranchType::Local).is_ok() {
    print_info(&format!("Switching to existing branch: {branch_name}",));
    return switch_to_branch(repo_path, branch_name);
  }

  // Branch doesn't exist
  if create_if_missing {
    print_info(&format!("Branch '{branch_name}' doesn't exist. Creating it...",));

    // Resolve parent branch
    let parent_branch = resolve_parent_branch(repo_path, parent_option)?;

    create_and_switch_to_branch(repo_path, branch_name, parent_branch.as_deref())
  } else {
    print_warning(&format!(
      "Branch '{branch_name}' doesn't exist. Use --create to create it.",
    ));
    Ok(())
  }
}

/// Switch to an existing branch
fn switch_to_branch(repo_path: &std::path::Path, branch_name: &str) -> Result<()> {
  let repo = Git2Repository::open(repo_path)?;

  // Find the branch
  let branch = repo
    .find_branch(branch_name, git2::BranchType::Local)
    .with_context(|| format!("Branch '{branch_name}' not found",))?;

  // Get the target commit
  let target = branch
    .get()
    .target()
    .ok_or_else(|| anyhow::anyhow!("Branch '{branch_name}' has no target commit",))?;

  // Set HEAD to the branch
  repo
    .set_head(&format!("refs/heads/{branch_name}",))
    .with_context(|| format!("Failed to set HEAD to branch '{branch_name}'",))?;

  let object = repo.find_object(target, None)?;

  // Checkout the branch
  let mut builder = git2::build::CheckoutBuilder::new();
  repo
    .checkout_tree(&object, Some(&mut builder))
    .with_context(|| format!("Failed to checkout branch '{branch_name}'",))?;

  print_success(&format!("Switched to branch '{branch_name}'",));
  Ok(())
}

/// Create a new branch and switch to it
fn create_and_switch_to_branch(
  repo_path: &std::path::Path,
  branch_name: &str,
  parent_branch: Option<&str>,
) -> Result<()> {
  let repo = Git2Repository::open(repo_path)?;

  // Get the HEAD commit to branch from
  let head = repo.head()?;
  let target = head
    .target()
    .ok_or_else(|| anyhow::anyhow!("HEAD is not a direct reference"))?;
  let commit = repo.find_commit(target)?;

  // Create the branch
  repo
    .branch(branch_name, &commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'",))?;

  print_success(&format!("Created branch '{branch_name}'",));

  // Switch to the new branch
  switch_to_branch(repo_path, branch_name)?;

  // Add dependency if parent is specified
  if let Some(parent) = parent_branch {
    add_branch_dependency(repo_path, branch_name, parent)?;
  }

  Ok(())
}

/// Add a branch dependency
fn add_branch_dependency(repo_path: &std::path::Path, child: &str, parent: &str) -> Result<()> {
  let mut repo_state = RepoState::load(repo_path)?;

  match repo_state.add_dependency(child.to_string(), parent.to_string()) {
    Ok(()) => {
      repo_state.save(repo_path)?;
      print_success(&format!("Added dependency: {child} -> {parent}"));
      Ok(())
    }
    Err(e) => {
      print_warning(&format!("Failed to add dependency: {e}"));
      Ok(()) // Continue despite dependency error
    }
  }
}

/// Create a branch from a Jira issue
fn create_branch_from_jira_issue(
  jira_client: &JiraClient,
  repo_path: &std::path::Path,
  issue_key: &str,
  parent_option: Option<&str>,
) -> Result<()> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  rt.block_on(async {
    // Fetch the issue to get its summary
    let issue = match jira_client.get_issue(issue_key).await {
      Ok(issue) => issue,
      Err(e) => {
        print_error(&format!("Failed to fetch issue {issue_key}: {e}"));
        return Err(e);
      }
    };

    // Create a branch name from the issue key and summary
    let summary = issue.fields.summary.to_lowercase();

    // Sanitize the summary for use in a branch name
    let sanitized_summary = summary
      .chars()
      .map(|c| match c {
        ' ' | '-' | '_' => '-',
        c if c.is_alphanumeric() => c,
        _ => '-',
      })
      .collect::<String>()
      .replace("--", "-")
      .trim_matches('-')
      .to_string();

    // Create the branch name in the format "PROJ-123/add-feature"
    let branch_name = format!("{issue_key}/{sanitized_summary}");

    print_info(&format!("Creating branch: {branch_name}",));

    // Resolve parent branch
    let parent_branch = resolve_parent_branch(repo_path, parent_option)?;

    // Create and switch to the branch
    create_and_switch_to_branch(repo_path, &branch_name, parent_branch.as_deref())?;

    // Store the association
    store_jira_association(repo_path, &branch_name, issue_key)?;

    print_success(&format!(
      "Created and switched to branch '{branch_name}' for Jira issue {issue_key}",
    ));
    Ok(())
  })
}

/// Create a branch from a GitHub PR
fn create_branch_from_github_pr(
  github_client: &GitHubClient,
  repo_path: &Path,
  pr_number: u32,
  parent_option: Option<&str>,
) -> Result<()> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  rt.block_on(async {
    // Open the git repository to get remote info
    let repo = Git2Repository::open(repo_path)?;
    let remote = repo.find_remote("origin")?;
    let remote_url = remote
      .url()
      .ok_or_else(|| anyhow::anyhow!("Failed to get remote URL"))?;

    // Extract owner and repo from remote URL
    let (owner, repo_name) = github_client.extract_repo_info_from_url(remote_url)?;

    // Get the PR details
    let pr = match github_client.get_pull_request(&owner, &repo_name, pr_number).await {
      Ok(pr) => pr,
      Err(e) => {
        print_error(&format!("Failed to get PR #{pr_number}: {e}",));
        return Err(e);
      }
    };

    // Use the PR's head branch name, but make it safe
    let branch_name = format!("pr-{pr_number}-{}", &pr.head.sha[..8]);

    print_info(&format!("Creating branch: {branch_name}",));

    // Resolve parent branch
    let parent_branch = resolve_parent_branch(repo_path, parent_option)?;

    // Create and switch to the branch
    create_and_switch_to_branch(repo_path, &branch_name, parent_branch.as_deref())?;

    // Store the association
    store_github_pr_association(repo_path, &branch_name, pr_number)?;

    print_success(&format!(
      "Created and switched to branch '{branch_name}' for GitHub PR #{pr_number}",
    ));
    print_info(&format!("PR Title: {}", pr.title));
    print_info(&format!("PR URL: {}", pr.html_url));
    Ok(())
  })
}

/// Store Jira issue association in repository state
fn store_jira_association(repo_path: &Path, branch_name: &str, issue_key: &str) -> Result<()> {
  let mut repo_state = RepoState::load(repo_path)?;

  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
    .unwrap()
    .to_rfc3339();

  repo_state.add_branch_issue(BranchMetadata {
    branch: branch_name.to_string(),
    jira_issue: Some(issue_key.to_string()),
    github_pr: None,
    created_at: time_str,
  });

  repo_state.save(repo_path)?;
  Ok(())
}

/// Store GitHub PR association in repository state
fn store_github_pr_association(repo_path: &Path, branch_name: &str, pr_number: u32) -> Result<()> {
  let mut repo_state = RepoState::load(repo_path)?;

  let now = chrono::Utc::now().to_rfc3339();

  repo_state.add_branch_issue(BranchMetadata {
    branch: branch_name.to_string(),
    jira_issue: None, // No Jira issue associated
    github_pr: Some(pr_number),
    created_at: now,
  });

  repo_state.save(repo_path)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_jira_issue_key() {
    assert!(is_jira_issue_key("PROJ-123"));
    assert!(is_jira_issue_key("ABC-456"));
    assert!(is_jira_issue_key("LONGPROJECT-999"));
    assert!(!is_jira_issue_key("proj-123")); // lowercase
    assert!(!is_jira_issue_key("P-123")); // too short
    assert!(!is_jira_issue_key("PROJ123")); // no hyphen
    assert!(!is_jira_issue_key("PROJ-")); // no number
    assert!(!is_jira_issue_key("123-PROJ")); // wrong order
  }

  #[test]
  fn test_extract_jira_issue_from_url() {
    assert_eq!(
      extract_jira_issue_from_url("https://company.atlassian.net/browse/PROJ-123"),
      Some("PROJ-123".to_string())
    );
    assert_eq!(
      extract_jira_issue_from_url("https://example.com/jira/browse/ABC-456"),
      Some("ABC-456".to_string())
    );
    assert_eq!(extract_jira_issue_from_url("https://example.com/other/page"), None);
  }

  #[test]
  fn test_extract_pr_number_from_url() {
    assert_eq!(
      extract_pr_number_from_url("https://github.com/owner/repo/pull/123").unwrap(),
      123
    );
    assert!(extract_pr_number_from_url("https://github.com/owner/repo").is_err());
  }

  #[test]
  fn test_detect_input_type() {
    // Jira issue keys
    if let InputType::JiraIssueKey(key) = detect_input_type("PROJ-123") {
      assert_eq!(key, "PROJ-123");
    } else {
      panic!("Expected JiraIssueKey");
    }

    // Jira URLs
    if let InputType::JiraIssueUrl(key) = detect_input_type("https://company.atlassian.net/browse/PROJ-123") {
      assert_eq!(key, "PROJ-123");
    } else {
      panic!("Expected JiraIssueUrl");
    }

    // GitHub PR IDs
    if let InputType::GitHubPrId(pr) = detect_input_type("123") {
      assert_eq!(pr, 123);
    } else {
      panic!("Expected GitHubPrId");
    }

    if let InputType::GitHubPrId(pr) = detect_input_type("PR#123") {
      assert_eq!(pr, 123);
    } else {
      panic!("Expected GitHubPrId");
    }

    // GitHub PR URLs
    if let InputType::GitHubPrUrl(pr) = detect_input_type("https://github.com/owner/repo/pull/123") {
      assert_eq!(pr, 123);
    } else {
      panic!("Expected GitHubPrUrl");
    }

    // Branch names
    if let InputType::BranchName(name) = detect_input_type("feature/my-branch") {
      assert_eq!(name, "feature/my-branch");
    } else {
      panic!("Expected BranchName");
    }
  }
}
