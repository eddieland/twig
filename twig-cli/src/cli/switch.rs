//! # Switch Command
//!
//! Derive-based implementation of the switch command for intelligently
//! switching to branches based on various inputs.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use clap::Args;
use directories::BaseDirs;
use git2::Repository as Git2Repository;
use regex::Regex;
use tokio::runtime::Runtime;
use twig_core::detect_repository;
use twig_core::jira_parser::JiraTicketParser;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::state::{BranchMetadata, RepoState};
use twig_gh::GitHubClient;
use twig_jira::JiraClient;

use crate::clients::{self, get_jira_host};

static GITHUB_PR_URL_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"github\.com/[^/]+/[^/]+/pull/(\d+)").expect("Failed to compile GitHub PR URL regex"));

static JIRA_ISSUE_URL_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"/browse/([A-Z]{2,}-\d+)").expect("Failed to compile Jira issue URL regex"));

/// Command for intelligently switching to branches based on various inputs
#[derive(Args)]
pub struct SwitchArgs {
  #[arg(
    required = false,
    index = 1,
    long_help = "Jira issue, GitHub PR, or branch name\n\n\
                Can be any of the following:\n\
                • Jira issue key (PROJ-123)\n\
                • Jira issue URL (https://company.atlassian.net/browse/PROJ-123)\n\
                • GitHub PR ID (12345 or PR#12345)\n\
                • GitHub PR URL (https://github.com/owner/repo/pull/123)\n\
                • Branch name (feature/my-branch)\n\n\
                Not required when using --root flag."
  )]
  pub input: Option<String>,

  #[arg(
    long = "root",
    long_help = "Switch to the current branch's dependency tree root\n\n\
                Traverses up the dependency chain from the current branch to find and switch to\n\
                the topmost parent branch. If the current branch has no dependencies, it will\n\
                remain on the current branch. This helps navigate to the root of a feature\n\
                branch dependency tree."
  )]
  pub root: bool,

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
    long_help = "Set parent dependency for the new branch (only applies when creating a new branch)\n\n\
               Specify a parent branch to create a dependency relationship when a new branch is created.\n\
               This option is ignored when switching to existing branches.\n\
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
/// branch name) and switches to the appropriate branch, or switches to the root
/// branch when the --root flag is used.
pub(crate) fn handle_switch_command(switch: SwitchArgs) -> Result<()> {
  let create_if_missing = !switch.no_create;
  let parent_option = switch.parent.as_deref();

  // Get the current repository
  let repo_path = detect_repository().context("Not in a git repository")?;

  // Create Jira parser once for the entire command
  let jira_parser = twig_core::create_jira_parser();

  // Handle --root flag
  if switch.root {
    if switch.input.is_some() {
      return Err(anyhow::anyhow!(
        "Cannot specify both --root flag and an input argument. Use either --root or provide an input."
      ));
    }
    return handle_root_switch(&repo_path);
  }

  // Require input if --root is not specified
  let input = match switch.input.as_ref() {
    Some(input) => input,
    None => {
      return Err(anyhow::anyhow!(
        "No input provided. Please specify a Jira issue, GitHub PR, or branch name.\nFor more information, run: twig switch --help"
      ));
    }
  };

  // Detect input type and handle accordingly
  match detect_input_type(jira_parser.as_ref(), input) {
    InputType::JiraIssueKey(issue_key) | InputType::JiraIssueUrl(issue_key) => {
      let jira_host = get_jira_host().context("Failed to get Jira host")?;

      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let jira = clients::create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host)
        .context("Failed to create Jira client")?;

      handle_jira_switch(
        &jira,
        &repo_path,
        &issue_key,
        create_if_missing,
        parent_option,
        jira_parser.as_ref(),
      )
    }
    InputType::GitHubPrId(pr_number) | InputType::GitHubPrUrl(pr_number) => {
      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let gh =
        clients::create_github_client_from_netrc(base_dirs.home_dir()).context("Failed to create GitHub client")?;

      handle_github_pr_switch(
        &gh,
        &repo_path,
        pr_number,
        create_if_missing,
        parent_option,
        jira_parser.as_ref(),
      )
    }
    InputType::BranchName(branch_name) => handle_branch_switch(
      &repo_path,
      &branch_name,
      create_if_missing,
      parent_option,
      jira_parser.as_ref(),
    ),
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
fn detect_input_type(jira_parser: Option<&JiraTicketParser>, input: &str) -> InputType {
  // Check for GitHub PR URL
  if input.contains("github.com")
    && input.contains("/pull/")
    && let Ok(pr_number) = extract_pr_number_from_url(input)
  {
    return InputType::GitHubPrUrl(pr_number);
  }

  // Check for Jira issue URL
  if (input.contains("atlassian.net/browse/") || (input.starts_with("http") && input.contains("/browse/")))
    && let Some(issue_key) = extract_jira_issue_from_url(input)
  {
    return InputType::JiraIssueUrl(issue_key);
  }

  // Check for GitHub PR ID patterns (123, PR#123, #123)
  let cleaned_input = input.trim_start_matches("PR#").trim_start_matches('#');
  if let Ok(pr_number) = cleaned_input.parse::<u32>() {
    return InputType::GitHubPrId(pr_number);
  }

  // Check for Jira issue key pattern (PROJ-123, ABC-456, etc.)
  if let Some(parser) = jira_parser
    && let Some(normalized_key) = parse_jira_issue_key(parser, input)
  {
    return InputType::JiraIssueKey(normalized_key);
  }

  // Default to branch name
  InputType::BranchName(input.to_string())
}

/// Parse and normalize a Jira issue key using the provided parser
fn parse_jira_issue_key(parser: &JiraTicketParser, input: &str) -> Option<String> {
  parser.parse(input).ok()
}

/// Extract PR number from GitHub URL
fn extract_pr_number_from_url(url: &str) -> Result<u32> {
  if let Some(captures) = GITHUB_PR_URL_REGEX.captures(url) {
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
  JIRA_ISSUE_URL_REGEX
    .captures(url)
    .and_then(|captures| captures.get(1))
    .map(|m| m.as_str().to_string())
}

/// Resolve parent branch based on the provided option
#[derive(Clone, Debug)]
enum BranchBase {
  Head,
  Parent { name: String },
}

impl BranchBase {
  fn parent_name(&self) -> Option<&str> {
    match self {
      BranchBase::Head => None,
      BranchBase::Parent { name } => Some(name.as_str()),
    }
  }
}

#[derive(Clone, Debug)]
struct BranchBaseResolution {
  base: BranchBase,
  commit: git2::Oid,
}

impl BranchBaseResolution {
  fn head(commit: git2::Oid) -> Self {
    Self {
      base: BranchBase::Head,
      commit,
    }
  }

  fn parent(name: String, commit: git2::Oid) -> Self {
    Self {
      base: BranchBase::Parent { name },
      commit,
    }
  }

  fn parent_name(&self) -> Option<&str> {
    self.base.parent_name()
  }
}

fn resolve_branch_base(
  repo_path: &std::path::Path,
  parent_option: Option<&str>,
  jira_parser: Option<&JiraTicketParser>,
) -> Result<BranchBaseResolution> {
  let repo = Git2Repository::open(repo_path)?;

  match parent_option.map(str::trim) {
    None | Some("") | Some("none") => {
      let head_commit = repo
        .head()
        .context("Failed to resolve HEAD for branch creation")?
        .peel_to_commit()
        .context("Failed to resolve HEAD commit for branch creation")?;
      Ok(BranchBaseResolution::head(head_commit.id()))
    }
    Some("current") => {
      let head = repo
        .head()
        .context("Failed to resolve current branch for --parent=current")?;

      if !head.is_branch() {
        return Err(anyhow::anyhow!(
          "HEAD is not on a branch. Create a branch first or use `--parent none`."
        ));
      }

      let branch_name = head.shorthand().unwrap_or_default().to_string();
      let commit = head
        .peel_to_commit()
        .context("Failed to resolve commit for the current branch")?
        .id();

      Ok(BranchBaseResolution::parent(branch_name, commit))
    }
    Some(parent) => {
      if let Some(parser) = jira_parser.as_ref()
        && let Some(normalized_key) = parse_jira_issue_key(parser, parent) {
          let repo_state = RepoState::load(repo_path)?;

          if let Some(branch_issue) = repo_state.get_branch_issue_by_jira(&normalized_key) {
            let commit = lookup_branch_tip(&repo, &branch_issue.branch)?
              .ok_or_else(|| parent_lookup_error(&branch_issue.branch))?;
            return Ok(BranchBaseResolution::parent(branch_issue.branch.clone(), commit));
          }
        }

      let commit = lookup_branch_tip(&repo, parent)?.ok_or_else(|| parent_lookup_error(parent))?;
      Ok(BranchBaseResolution::parent(parent.to_string(), commit))
    }
  }
}

fn lookup_branch_tip(repo: &Git2Repository, branch_name: &str) -> Result<Option<git2::Oid>> {
  if let Ok(branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
    let commit = branch
      .into_reference()
      .peel_to_commit()
      .context("Failed to resolve local branch commit")?;
    return Ok(Some(commit.id()));
  }

  let remote_branch_name = format!("origin/{branch_name}");
  if let Ok(branch) = repo.find_branch(&remote_branch_name, git2::BranchType::Remote) {
    let commit = branch
      .into_reference()
      .peel_to_commit()
      .context("Failed to resolve remote branch commit")?;
    return Ok(Some(commit.id()));
  }

  if let Ok(mut remote) = repo.find_remote("origin") {
    let mut fetch_options = git2::FetchOptions::new();
    remote
      .fetch(&[branch_name], Some(&mut fetch_options), None)
      .with_context(|| format!("Failed to fetch '{branch_name}' from origin"))?;

    if let Ok(branch) = repo.find_branch(&remote_branch_name, git2::BranchType::Remote) {
      let commit = branch
        .into_reference()
        .peel_to_commit()
        .context("Failed to resolve fetched branch commit")?;
      return Ok(Some(commit.id()));
    }
  }

  Ok(None)
}

fn parent_lookup_error(parent: &str) -> anyhow::Error {
  anyhow::anyhow!(
    "Parent branch '{parent}' was not found. Create it first and record dependencies with `twig branch depend` or `twig branch root add`."
  )
}

/// Handle switching to a branch based on Jira issue
fn handle_jira_switch(
  jira: &JiraClient,
  repo_path: &Path,
  issue_key: &str,
  create_if_missing: bool,
  parent_option: Option<&str>,
  jira_parser: Option<&JiraTicketParser>,
) -> Result<()> {
  tracing::info!("Looking for branch associated with Jira issue: {}", issue_key);

  // Load repository state to find associated branch
  let repo_state = RepoState::load(repo_path)?;

  // Look for existing branch association
  if let Some(branch_issue) = repo_state.get_branch_issue_by_jira(issue_key) {
    let branch_name = &branch_issue.branch;
    tracing::info!("Found associated branch: {}", branch_name);
    return switch_to_branch(repo_path, branch_name);
  }

  // No existing association found
  if create_if_missing {
    print_info("No associated branch found. Creating new branch from Jira issue...");
    create_branch_from_jira_issue(jira, repo_path, issue_key, parent_option, jira_parser)
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
  jira_parser: Option<&JiraTicketParser>,
) -> Result<()> {
  tracing::info!("Looking for branch associated with GitHub PR: #{}", pr_number);

  // Load repository state to find associated branch
  let repo_state = RepoState::load(repo_path)?;

  // Look for existing branch association
  for branch_issue in repo_state.list_branch_issues() {
    if let Some(github_pr) = branch_issue.github_pr
      && github_pr == pr_number
    {
      let branch_name = &branch_issue.branch;
      tracing::info!("Found associated branch: {}", branch_name);
      return switch_to_branch(repo_path, branch_name);
    }
  }

  // No existing association found
  if create_if_missing {
    print_info("No associated branch found. Creating new branch from GitHub PR...");
    create_branch_from_github_pr(gh, repo_path, pr_number, parent_option, jira_parser)
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
  jira_parser: Option<&JiraTicketParser>,
) -> Result<()> {
  let repo = Git2Repository::open(repo_path)?;

  // Check if branch exists
  if repo.find_branch(branch_name, git2::BranchType::Local).is_ok() {
    tracing::info!("Switching to existing branch: {}", branch_name);
    return switch_to_branch(repo_path, branch_name);
  }

  // Branch doesn't exist
  if create_if_missing {
    print_info(&format!("Branch '{branch_name}' doesn't exist. Creating it...",));

    // Resolve parent branch
    let branch_base = resolve_branch_base(repo_path, parent_option, jira_parser)?;

    create_and_switch_to_branch(repo_path, branch_name, &branch_base)
  } else {
    print_warning(&format!(
      "Branch '{branch_name}' doesn't exist. Use --create to create it.",
    ));
    Ok(())
  }
}

/// Handle switching to the root branch
fn handle_root_switch(repo_path: &std::path::Path) -> Result<()> {
  tracing::info!("Looking for current branch's dependency tree root");

  // Get the current branch
  let repo = Git2Repository::open(repo_path)?;
  let head = repo.head()?;

  if !head.is_branch() {
    return Err(anyhow::anyhow!(
      "HEAD is not on a branch. Cannot determine dependency tree root."
    ));
  }

  let current_branch = head.shorthand().unwrap_or_default();
  tracing::info!("Current branch: {}", current_branch);

  // Load repository state to find dependency tree root
  let repo_state = RepoState::load(repo_path)?;

  // Find the root of the current branch's dependency tree
  let dependency_root = repo_state.find_dependency_tree_root(current_branch);
  tracing::info!("Found dependency tree root: {}", dependency_root);

  // If the dependency root is the same as current branch, we're already at the
  // root
  if dependency_root == current_branch {
    print_info(&format!("Already at dependency tree root: {current_branch}"));
    return Ok(());
  }

  // Check if the root branch actually exists
  if repo.find_branch(&dependency_root, git2::BranchType::Local).is_err() {
    return Err(anyhow::anyhow!(
      "Dependency tree root branch '{}' does not exist locally.\n\
       This may indicate a broken dependency chain.",
      dependency_root
    ));
  }

  switch_to_branch(repo_path, &dependency_root)
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
  branch_base: &BranchBaseResolution,
) -> Result<()> {
  let repo = Git2Repository::open(repo_path)?;

  let base_commit = repo
    .find_commit(branch_base.commit)
    .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

  repo
    .branch(branch_name, &base_commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'",))?;

  print_success(&format!("Created branch '{branch_name}'",));

  switch_to_branch(repo_path, branch_name)?;

  if let Some(parent) = branch_base.parent_name() {
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
  jira_parser: Option<&JiraTicketParser>,
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
    let branch_base = resolve_branch_base(repo_path, parent_option, jira_parser)?;

    // Create and switch to the branch
    create_and_switch_to_branch(repo_path, &branch_name, &branch_base)?;

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
  jira_parser: Option<&JiraTicketParser>,
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
    let branch_base = resolve_branch_base(repo_path, parent_option, jira_parser)?;

    // Create and switch to the branch
    create_and_switch_to_branch(repo_path, &branch_name, &branch_base)?;

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
  use anyhow::Result;
  use git2::BranchType;
  use serde_json::json;
  use tokio::runtime::Runtime;
  use twig_test_utils::{GitRepoTestGuard, checkout_branch, create_commit, setup_test_env_with_init};
  use wiremock::matchers::{method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::*;

  #[test]
  fn test_parse_jira_issue_key() {
    use twig_core::jira_parser::{JiraParsingConfig, JiraTicketParser};

    let parser = JiraTicketParser::new(JiraParsingConfig::default());

    assert_eq!(parse_jira_issue_key(&parser, "PROJ-123"), Some("PROJ-123".to_string()));
    assert_eq!(parse_jira_issue_key(&parser, "ABC-456"), Some("ABC-456".to_string()));
    assert_eq!(
      parse_jira_issue_key(&parser, "LONGPROJECT-999"),
      Some("LONGPROJECT-999".to_string())
    );
    assert_eq!(parse_jira_issue_key(&parser, "PROJ-"), None); // no number
    assert_eq!(parse_jira_issue_key(&parser, "123-PROJ"), None); // wrong order
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
    use twig_core::jira_parser::{JiraParsingConfig, JiraTicketParser};

    let parser = JiraTicketParser::new(JiraParsingConfig::default());

    // Jira issue keys
    if let InputType::JiraIssueKey(key) = detect_input_type(Some(&parser), "PROJ-123") {
      assert_eq!(key, "PROJ-123");
    } else {
      panic!("Expected JiraIssueKey");
    }

    // Jira URLs
    if let InputType::JiraIssueUrl(key) =
      detect_input_type(Some(&parser), "https://company.atlassian.net/browse/PROJ-123")
    {
      assert_eq!(key, "PROJ-123");
    } else {
      panic!("Expected JiraIssueUrl");
    }

    // GitHub PR IDs
    if let InputType::GitHubPrId(pr) = detect_input_type(Some(&parser), "123") {
      assert_eq!(pr, 123);
    } else {
      panic!("Expected GitHubPrId");
    }

    if let InputType::GitHubPrId(pr) = detect_input_type(Some(&parser), "PR#123") {
      assert_eq!(pr, 123);
    } else {
      panic!("Expected GitHubPrId");
    }

    // GitHub PR URLs
    if let InputType::GitHubPrUrl(pr) = detect_input_type(Some(&parser), "https://github.com/owner/repo/pull/123") {
      assert_eq!(pr, 123);
    } else {
      panic!("Expected GitHubPrUrl");
    }

    // Branch names
    if let InputType::BranchName(name) = detect_input_type(Some(&parser), "feature/my-branch") {
      assert_eq!(name, "feature/my-branch");
    } else {
      panic!("Expected BranchName");
    }
  }

  #[test]
  fn test_create_branch_from_parent_tip() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;

    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("parent", &head_commit, true)?;
    checkout_branch(repo, "parent")?;
    create_commit(repo, "parent.txt", "parent", "parent commit")?;
    let parent_tip = repo.head()?.peel_to_commit()?.id();

    let branch_base = resolve_branch_base(repo_guard.path(), Some("parent"), None)?;
    create_and_switch_to_branch(repo_guard.path(), "feature/new", &branch_base)?;

    let created_branch = repo.find_branch("feature/new", BranchType::Local)?;
    let created_tip = created_branch.into_reference().peel_to_commit()?.id();
    assert_eq!(created_tip, parent_tip);

    let repo_state = RepoState::load(repo_guard.path())?;
    let dependency = repo_state
      .dependencies
      .iter()
      .find(|dep| dep.child == "feature/new")
      .expect("dependency recorded");
    assert_eq!(dependency.parent, "parent");

    Ok(())
  }

  #[test]
  fn test_parent_branch_missing_errors() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;

    let err =
      resolve_branch_base(repo_guard.path(), Some("missing"), None).expect_err("expected missing parent to error");
    assert!(err.to_string().contains("twig branch depend"));

    Ok(())
  }

  #[test]
  fn test_create_branch_from_jira_issue_uses_parent_tip() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("parent", &head_commit, true)?;
    checkout_branch(repo, "parent")?;
    create_commit(repo, "parent.txt", "parent", "parent commit")?;
    let parent_tip = repo.head()?.peel_to_commit()?.id();

    let runtime = Runtime::new()?;
    let mock_server = runtime.block_on(MockServer::start());

    runtime.block_on(async {
      Mock::given(method("GET"))
        .and(path("/rest/api/2/issue/PROJ-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
          "id": "100",
          "key": "PROJ-123",
          "fields": {
            "summary": "Example Feature",
            "description": "Example description",
            "status": { "name": "In Progress" }
          }
        })))
        .mount(&mock_server)
        .await;
    });

    let jira_client = JiraClient::new(
      &mock_server.uri(),
      twig_jira::models::JiraAuth {
        username: "user".to_string(),
        api_token: "token".to_string(),
      },
    );

    create_branch_from_jira_issue(&jira_client, repo_guard.path(), "PROJ-123", Some("parent"), None)?;

    let created_branch = repo.find_branch("PROJ-123/example-feature", BranchType::Local)?;
    let created_tip = created_branch.into_reference().peel_to_commit()?.id();
    assert_eq!(created_tip, parent_tip);

    let repo_state = RepoState::load(repo_guard.path())?;
    let dependency = repo_state
      .dependencies
      .iter()
      .find(|dep| dep.child == "PROJ-123/example-feature")
      .expect("dependency recorded");
    assert_eq!(dependency.parent, "parent");

    let metadata = repo_state
      .get_branch_metadata("PROJ-123/example-feature")
      .expect("metadata recorded");
    assert_eq!(metadata.jira_issue.as_deref(), Some("PROJ-123"));

    Ok(())
  }

  #[test]
  fn test_create_branch_from_github_pr_respects_parent_tip() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("parent", &head_commit, true)?;
    checkout_branch(repo, "parent")?;
    create_commit(repo, "parent.txt", "parent", "parent commit")?;
    let parent_tip = repo.head()?.peel_to_commit()?.id();

    repo.remote("origin", "https://github.com/example/repo.git")?;

    let runtime = Runtime::new()?;
    let mock_server = runtime.block_on(MockServer::start());

    runtime.block_on(async {
      Mock::given(method("GET"))
        .and(path("/repos/example/repo/pulls/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
          "number": 42,
          "title": "Example PR",
          "html_url": "https://github.com/example/repo/pull/42",
          "state": "open",
          "user": { "login": "octocat", "id": 1, "name": "Octocat" },
          "created_at": "2021-01-01T00:00:00Z",
          "updated_at": "2021-01-01T00:00:00Z",
          "head": {
            "label": "octocat:feature",
            "ref_name": "feature",
            "sha": "abcdef1234567890abcdef1234567890abcdef12"
          },
          "base": {
            "label": "octocat:main",
            "ref_name": "main",
            "sha": "1234567890abcdef1234567890abcdef12345678"
          },
          "mergeable": true,
          "mergeable_state": "clean",
          "draft": false
        })))
        .mount(&mock_server)
        .await;
    });

    let mut github_client = twig_gh::GitHubClient::new(twig_gh::models::GitHubAuth {
      username: "user".to_string(),
      token: "token".to_string(),
    });
    github_client.set_base_url(mock_server.uri());

    create_branch_from_github_pr(&github_client, repo_guard.path(), 42, Some("parent"), None)?;

    let created_branch = repo.find_branch("pr-42-abcdef12", BranchType::Local)?;
    let created_tip = created_branch.into_reference().peel_to_commit()?.id();
    assert_eq!(created_tip, parent_tip);

    let repo_state = RepoState::load(repo_guard.path())?;
    let dependency = repo_state
      .dependencies
      .iter()
      .find(|dep| dep.child == "pr-42-abcdef12")
      .expect("dependency recorded");
    assert_eq!(dependency.parent, "parent");

    let metadata = repo_state
      .get_branch_metadata("pr-42-abcdef12")
      .expect("metadata recorded");
    assert_eq!(metadata.github_pr, Some(42));

    Ok(())
  }
}
