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
use twig_core::git::switch::{
  BranchBaseResolution, ParentBranchOption, PullRequestCheckoutRequest, PullRequestHeadInfo, SwitchInput,
  checkout_pr_branch, detect_switch_input, resolve_branch_base, store_jira_association, try_checkout_remote_branch,
};
use twig_core::jira_parser::JiraTicketParser;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::state::RepoState;
use twig_core::{checkout_branch, detect_repository, generate_branch_name_from_issue};
use twig_gh::{GitHubClient, GitHubRepo, create_github_client_from_netrc};
use twig_jira::{JiraClient, create_jira_client_from_netrc, get_jira_host};

use crate::complete::switch_target_completer;

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
                Not required when using --root flag.",
    add = switch_target_completer()
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

/// Context for switch operations
struct SwitchContext<'a> {
  repo: &'a Git2Repository,
  repo_path: &'a Path,
  repo_state: &'a RepoState,
  create_if_missing: bool,
  parent_option: ParentBranchOption,
  jira_parser: Option<&'a JiraTicketParser>,
}

/// Handle the switch command
///
/// This function detects the type of input provided (Jira issue, GitHub PR, or
/// branch name) and switches to the appropriate branch, or switches to the root
/// branch when the --root flag is used.
pub(crate) fn handle_switch_command(switch: SwitchArgs) -> Result<()> {
  let create_if_missing = !switch.no_create;
  let parent_option = ParentBranchOption::from_cli_value(switch.parent.as_deref());

  // Get the current repository
  let repo_path = detect_repository().context("Not in a git repository")?;
  let repo = Git2Repository::open(&repo_path)?;
  let repo_state = RepoState::load(&repo_path)?;

  // Create Jira parser once for the entire command
  let jira_parser = twig_core::create_jira_parser();

  // Handle --root flag
  if switch.root {
    if switch.input.is_some() {
      return Err(anyhow::anyhow!(
        "Cannot specify both --root flag and an input argument. Use either --root or provide an input."
      ));
    }
    return handle_root_switch(&repo, &repo_path, &repo_state);
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

  // Create context for switch operations
  let ctx = SwitchContext {
    repo: &repo,
    repo_path: &repo_path,
    repo_state: &repo_state,
    create_if_missing,
    parent_option,
    jira_parser: jira_parser.as_ref(),
  };

  // Detect input type and handle accordingly
  match detect_switch_input(jira_parser.as_ref(), input) {
    SwitchInput::JiraIssueKey(issue_key) | SwitchInput::JiraIssueUrl(issue_key) => {
      let jira_host = get_jira_host().context("Failed to get Jira host")?;

      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let jira =
        create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host).context("Failed to create Jira client")?;

      handle_jira_switch(&jira, &ctx, &issue_key)
    }
    SwitchInput::GitHubPrId(pr_number) | SwitchInput::GitHubPrUrl(pr_number) => {
      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let gh = create_github_client_from_netrc(base_dirs.home_dir()).context("Failed to create GitHub client")?;

      handle_github_pr_switch(&gh, &ctx, pr_number)
    }
    SwitchInput::BranchName(branch_name) => handle_branch_switch(
      ctx.repo,
      ctx.repo_path,
      &branch_name,
      ctx.create_if_missing,
      &ctx.parent_option,
      ctx.jira_parser,
    ),
    _ => unreachable!("Unhandled switch input variant"),
  }
}

/// Handle switching to a branch based on Jira issue
fn handle_jira_switch(jira: &JiraClient, ctx: &SwitchContext, issue_key: &str) -> Result<()> {
  tracing::info!("Looking for branch associated with Jira issue: {}", issue_key);

  // Look for existing branch association
  if let Some(branch_issue) = ctx.repo_state.get_branch_issue_by_jira(issue_key) {
    let branch_name = &branch_issue.branch;
    tracing::info!("Found associated branch: {}", branch_name);
    return switch_to_branch(ctx.repo, ctx.repo_path, branch_name);
  }

  // No existing association found
  if ctx.create_if_missing {
    print_info("No associated branch found. Creating new branch from Jira issue...");
    create_branch_from_jira_issue(
      jira,
      ctx.repo,
      ctx.repo_path,
      issue_key,
      &ctx.parent_option,
      ctx.jira_parser,
    )
  } else {
    print_warning(&format!(
      "No branch found for Jira issue {issue_key}. Use --create to create a new branch.",
    ));
    Ok(())
  }
}

/// Handle switching to a branch based on GitHub PR
fn handle_github_pr_switch(gh: &GitHubClient, ctx: &SwitchContext, pr_number: u32) -> Result<()> {
  tracing::info!("Looking for branch associated with GitHub PR: #{}", pr_number);

  // Look for existing branch association
  for branch_issue in ctx.repo_state.list_branch_issues() {
    if let Some(github_pr) = branch_issue.github_pr
      && github_pr == pr_number
    {
      let branch_name = &branch_issue.branch;
      tracing::info!("Found associated branch: {}", branch_name);
      return switch_to_branch(ctx.repo, ctx.repo_path, branch_name);
    }
  }

  // No existing association found
  if ctx.create_if_missing {
    print_info("No associated branch found. Creating new branch from GitHub PR...");
    create_branch_from_github_pr(gh, ctx.repo, ctx.repo_path, pr_number, &ctx.parent_option)
  } else {
    print_warning(&format!(
      "No branch found for GitHub PR #{pr_number}. Use --create to create a new branch.",
    ));
    Ok(())
  }
}

/// Handle switching to a branch by name
fn handle_branch_switch(
  repo: &Git2Repository,
  repo_path: &std::path::Path,
  branch_name: &str,
  create_if_missing: bool,
  parent_option: &ParentBranchOption,
  jira_parser: Option<&JiraTicketParser>,
) -> Result<()> {
  // Check if branch exists
  if repo.find_branch(branch_name, git2::BranchType::Local).is_ok() {
    tracing::info!("Switching to existing branch: {}", branch_name);
    return switch_to_branch(repo, repo_path, branch_name);
  }

  // Branch doesn't exist
  if create_if_missing {
    if try_checkout_remote_branch(repo, branch_name)? {
      print_success(&format!("Checked out {branch_name} from origin.",));
      return Ok(());
    }

    print_info(&format!("Branch '{branch_name}' doesn't exist. Creating it...",));

    // Resolve parent branch
    let branch_base = resolve_branch_base(repo, repo_path, parent_option, jira_parser)?;

    create_and_switch_to_branch(repo, repo_path, branch_name, &branch_base)
  } else {
    print_warning(&format!(
      "Branch '{branch_name}' doesn't exist. Use --create to create it.",
    ));
    Ok(())
  }
}

/// Handle switching to the root branch
fn handle_root_switch(repo: &Git2Repository, repo_path: &std::path::Path, repo_state: &RepoState) -> Result<()> {
  tracing::info!("Looking for current branch's dependency tree root");

  // Get the current branch
  let head = repo.head()?;

  if !head.is_branch() {
    return Err(anyhow::anyhow!(
      "HEAD is not on a branch. Cannot determine dependency tree root."
    ));
  }

  let current_branch = head.shorthand().unwrap_or_default();
  tracing::info!("Current branch: {}", current_branch);

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
      "Dependency tree root branch '{dependency_root}' does not exist locally.\n\
       This may indicate a broken dependency chain."
    ));
  }

  switch_to_branch(repo, repo_path, &dependency_root)
}

/// Switch to an existing branch
fn switch_to_branch(repo: &Git2Repository, _repo_path: &std::path::Path, branch_name: &str) -> Result<()> {
  checkout_branch(repo, branch_name)?;
  print_success(&format!("Switched to branch '{branch_name}'",));
  Ok(())
}

/// Create a new branch and switch to it
fn create_and_switch_to_branch(
  repo: &Git2Repository,
  repo_path: &std::path::Path,
  branch_name: &str,
  branch_base: &BranchBaseResolution,
) -> Result<()> {
  let base_commit = repo
    .find_commit(branch_base.commit())
    .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

  repo
    .branch(branch_name, &base_commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'",))?;

  print_success(&format!("Created branch '{branch_name}'",));

  switch_to_branch(repo, repo_path, branch_name)?;

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
  repo: &Git2Repository,
  repo_path: &std::path::Path,
  issue_key: &str,
  parent_option: &ParentBranchOption,
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

    // Create a branch name from the issue key and summary (without stop word filtering)
    let branch_name = generate_branch_name_from_issue(issue_key, &issue.fields.summary, false);

    print_info(&format!("Creating branch: {branch_name}",));

    // Resolve parent branch
    let branch_base = resolve_branch_base(repo, repo_path, parent_option, jira_parser)?;

    // Create and switch to the branch
    create_and_switch_to_branch(repo, repo_path, &branch_name, &branch_base)?;

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
  repo: &Git2Repository,
  repo_path: &Path,
  pr_number: u32,
  parent_option: &ParentBranchOption,
) -> Result<()> {
  let rt = Runtime::new().context("Failed to create async runtime")?;
  rt.block_on(async {
    // Get remote info
    let remote = repo.find_remote("origin")?;
    let remote_url = remote
      .url()
      .ok_or_else(|| anyhow::anyhow!("Failed to get remote URL"))?;

    // Extract owner and repo from remote URL
    let github_repo = GitHubRepo::parse(remote_url)?;
    let (owner, repo_name) = (github_repo.owner, github_repo.repo);

    // Get the PR details
    let pr = match github_client.get_pull_request(&owner, &repo_name, pr_number).await {
      Ok(pr) => pr,
      Err(e) => {
        print_error(&format!("Failed to get PR #{pr_number}: {e}",));
        return Err(e);
      }
    };

    let branch_name = pr
      .head
      .ref_name
      .clone()
      .or_else(|| pr.head.label.split(':').nth(1).map(|s| s.to_string()))
      .ok_or_else(|| anyhow::anyhow!("Pull request is missing a head branch name"))?;

    print_info(&format!("Creating branch from PR head: {branch_name}"));

    // Map twig-gh types to core-native types
    let head_info = PullRequestHeadInfo {
      branch: branch_name,
      repo_full_name: pr.head.repo.as_ref().and_then(|r| r.full_name.clone()),
      owner_login: pr
        .head
        .repo
        .as_ref()
        .and_then(|r| r.owner.as_ref().map(|o| o.login.clone())),
      ssh_url: pr.head.repo.as_ref().and_then(|r| r.ssh_url.clone()),
      clone_url: pr.head.repo.as_ref().and_then(|r| r.clone_url.clone()),
    };

    let request = PullRequestCheckoutRequest {
      pr_number,
      head: head_info,
      origin_url: remote_url.to_string(),
      origin_owner: owner,
      origin_repo: repo_name,
      parent: match parent_option {
        ParentBranchOption::Head => None,
        ParentBranchOption::CurrentBranch => repo.head().ok().and_then(|h| h.shorthand().map(|s| s.to_string())),
        ParentBranchOption::Named(name) => Some(name.clone()),
      },
    };

    let outcome = checkout_pr_branch(repo, repo_path, &request)?;

    if outcome.fork_remote_created
      && let Some(url) = &outcome.fork_remote_url
    {
      print_info(&format!(
        "Added remote '{}' for PR head repository: {url}",
        outcome.remote_name
      ));
    }

    print_success(&format!(
      "Created and switched to branch '{}' for GitHub PR #{pr_number}",
      outcome.branch_name,
    ));
    print_info(&format!("PR Title: {}", pr.title));
    print_info(&format!("PR URL: {}", pr.html_url));
    Ok(())
  })
}

#[cfg(test)]
mod tests {
  use std::env;

  use anyhow::Result;
  use git2::BranchType;
  use serde_json::json;
  use tokio::runtime::Runtime;
  use twig_test_utils::{GitRepoTestGuard, checkout_branch, create_commit, setup_test_env_with_init};
  use wiremock::matchers::{method, path};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::*;

  struct DirGuard {
    original: std::path::PathBuf,
  }

  impl DirGuard {
    fn new() -> Self {
      let original = env::current_dir().expect("Failed to read current directory");
      Self { original }
    }
  }

  impl Drop for DirGuard {
    fn drop(&mut self) {
      let _ = env::set_current_dir(&self.original);
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

    let branch_base = resolve_branch_base(
      repo,
      repo_guard.path(),
      &ParentBranchOption::Named("parent".into()),
      None,
    )?;
    create_and_switch_to_branch(repo, repo_guard.path(), "feature/new", &branch_base)?;

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
  fn test_switch_to_remote_branch_creates_tracking_branch() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;

    repo.remote("origin", "https://github.com/example/repo.git")?;

    let head_commit = repo.head()?.peel_to_commit()?.id();
    repo.reference(
      "refs/remotes/origin/feature/existing",
      head_commit,
      true,
      "test remote branch",
    )?;

    assert!(try_checkout_remote_branch(&repo, "feature/existing")?);

    let repo = git2::Repository::open(repo_guard.path())?;
    let local_branch = repo.find_branch("feature/existing", BranchType::Local)?;
    assert_eq!(repo.head()?.shorthand(), Some("feature/existing"));

    let tip = local_branch
      .get()
      .target()
      .expect("local branch should have a target commit");
    assert_eq!(tip, head_commit);

    let upstream = local_branch.upstream()?;
    assert_eq!(upstream.name()?, Some("origin/feature/existing"));

    Ok(())
  }

  #[test]
  fn test_parent_branch_missing_errors() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;

    let err = resolve_branch_base(
      repo,
      repo_guard.path(),
      &ParentBranchOption::Named("missing".into()),
      None,
    )
    .expect_err("expected missing parent to error");
    assert!(err.to_string().contains("twig branch depend"));

    Ok(())
  }

  #[test]
  fn test_create_branch_from_jira_issue_uses_parent_tip() -> Result<()> {
    let _dir_guard = DirGuard::new();
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

    create_branch_from_jira_issue(
      &jira_client,
      repo,
      repo_guard.path(),
      "PROJ-123",
      &ParentBranchOption::Named("parent".into()),
      None,
    )?;

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
  fn test_create_branch_from_github_pr_checks_out_head_commit() -> Result<()> {
    let _dir_guard = DirGuard::new();
    use std::fs;

    use tempfile::TempDir;

    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;

    let remote_root = TempDir::new()?;
    let remote_repo_path = remote_root.path().join("github.com/example/repo");
    fs::create_dir_all(&remote_repo_path)?;
    let remote_repo = git2::Repository::init(&remote_repo_path)?;
    let mut remote_config = remote_repo.config()?;
    remote_config.set_str("user.name", "Twig Test User")?;
    remote_config.set_str("user.email", "twig-test@example.com")?;

    create_commit(&remote_repo, "base.txt", "base", "base commit")?;
    let initial = remote_repo.head()?.peel_to_commit()?;
    remote_repo.branch("parent", &initial, true)?;
    remote_repo.branch("feature/cool", &initial, true)?;

    checkout_branch(&remote_repo, "feature/cool")?;
    create_commit(&remote_repo, "feature.txt", "feature", "feature commit")?;
    let pr_head_oid = remote_repo.head()?.peel_to_commit()?.id();

    checkout_branch(&remote_repo, "parent")?;
    create_commit(&remote_repo, "parent.txt", "parent", "parent commit")?;

    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;
    repo.remote("origin", remote_repo_path.to_str().unwrap())?;

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
            "label": "octocat:feature/cool",
            "ref": "feature/cool",
            "sha": pr_head_oid.to_string(),
            "repo": {
              "full_name": "example/repo",
              "clone_url": "https://github.com/example/repo.git",
              "ssh_url": "git@github.com:example/repo.git",
              "owner": { "login": "octocat", "id": 1, "name": "Octocat" }
            }
          },
          "base": {
            "label": "octocat:parent",
            "ref": "parent",
            "sha": remote_repo.refname_to_id("refs/heads/parent").unwrap().to_string(),
            "repo": {
              "full_name": "example/repo",
              "clone_url": "https://github.com/example/repo.git",
              "ssh_url": "git@github.com:example/repo.git",
              "owner": { "login": "octocat", "id": 1, "name": "Octocat" }
            }
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

    create_branch_from_github_pr(
      &github_client,
      repo,
      repo_guard.path(),
      42,
      &ParentBranchOption::Named("parent".into()),
    )?;

    let created_branch = repo.find_branch("feature/cool", BranchType::Local)?;
    let created_tip = created_branch.into_reference().peel_to_commit()?.id();
    assert_eq!(created_tip, pr_head_oid);

    let upstream = repo.find_branch("feature/cool", BranchType::Local)?.upstream()?;
    assert_eq!(upstream.name().unwrap(), Some("origin/feature/cool"));

    let repo_state = RepoState::load(repo_guard.path())?;
    let dependency = repo_state
      .dependencies
      .iter()
      .find(|dep| dep.child == "feature/cool")
      .expect("dependency recorded");
    assert_eq!(dependency.parent, "parent");

    let metadata = repo_state
      .get_branch_metadata("feature/cool")
      .expect("metadata recorded");
    assert_eq!(metadata.github_pr, Some(42));

    Ok(())
  }

  #[test]
  fn test_create_branch_from_github_pr_handles_fork_head() -> Result<()> {
    let _dir_guard = DirGuard::new();
    use std::fs;

    use tempfile::TempDir;

    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;

    let origin_root = TempDir::new()?;
    let origin_repo_path = origin_root.path().join("github.com/example/repo");
    fs::create_dir_all(&origin_repo_path)?;
    let origin_repo = git2::Repository::init(&origin_repo_path)?;
    let mut origin_config = origin_repo.config()?;
    origin_config.set_str("user.name", "Twig Test User")?;
    origin_config.set_str("user.email", "twig-test@example.com")?;

    create_commit(&origin_repo, "base.txt", "base", "base commit")?;
    let base_commit = origin_repo.head()?.peel_to_commit()?;
    origin_repo.branch("parent", &base_commit, true)?;

    checkout_branch(&origin_repo, "parent")?;
    create_commit(&origin_repo, "parent.txt", "parent", "parent commit")?;

    let fork_root = TempDir::new()?;
    let fork_repo_path = fork_root.path().join("github.com/forker/repo");
    fs::create_dir_all(&fork_repo_path)?;
    let fork_repo = git2::Repository::init(&fork_repo_path)?;
    let mut fork_config = fork_repo.config()?;
    fork_config.set_str("user.name", "Twig Test User")?;
    fork_config.set_str("user.email", "twig-test@example.com")?;

    create_commit(&fork_repo, "base.txt", "base", "base commit")?;
    let fork_base = fork_repo.head()?.peel_to_commit()?;
    fork_repo.branch("feature/cool", &fork_base, true)?;
    checkout_branch(&fork_repo, "feature/cool")?;
    create_commit(&fork_repo, "feature.txt", "feature", "feature commit")?;
    let fork_head_oid = fork_repo.head()?.peel_to_commit()?.id();

    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;
    repo.remote("origin", origin_repo_path.to_str().unwrap())?;

    let runtime = Runtime::new()?;
    let mock_server = runtime.block_on(MockServer::start());

    runtime.block_on(async {
      Mock::given(method("GET"))
        .and(path("/repos/example/repo/pulls/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
          "number": 99,
          "title": "Forked PR",
          "html_url": "https://github.com/example/repo/pull/99",
          "state": "open",
          "user": { "login": "octocat", "id": 1, "name": "Octocat" },
          "created_at": "2021-01-01T00:00:00Z",
          "updated_at": "2021-01-01T00:00:00Z",
          "head": {
            "label": "forker:feature/cool",
            "ref": "feature/cool",
            "sha": fork_head_oid.to_string(),
            "repo": {
              "full_name": "forker/repo",
              "clone_url": fork_repo_path.to_str().unwrap(),
              "ssh_url": fork_repo_path.to_str().unwrap(),
              "owner": { "login": "forker", "id": 2, "name": "Forker" }
            }
          },
          "base": {
            "label": "octocat:parent",
            "ref": "parent",
            "sha": origin_repo.refname_to_id("refs/heads/parent").unwrap().to_string(),
            "repo": {
              "full_name": "example/repo",
              "clone_url": "https://github.com/example/repo.git",
              "ssh_url": "git@github.com:example/repo.git",
              "owner": { "login": "octocat", "id": 1, "name": "Octocat" }
            }
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

    create_branch_from_github_pr(
      &github_client,
      repo,
      repo_guard.path(),
      99,
      &ParentBranchOption::Named("parent".into()),
    )?;

    let created_branch = repo.find_branch("feature/cool", BranchType::Local)?;
    let created_tip = created_branch.into_reference().peel_to_commit()?.id();
    assert_eq!(created_tip, fork_head_oid);

    let upstream = repo.find_branch("feature/cool", BranchType::Local)?.upstream()?;
    assert_eq!(upstream.name().unwrap(), Some("fork-forker/feature/cool"));

    let fork_remote = repo.find_remote("fork-forker")?;
    assert_eq!(fork_remote.url(), Some(fork_repo_path.to_str().unwrap()));

    let repo_state = RepoState::load(repo_guard.path())?;
    let metadata = repo_state
      .get_branch_metadata("feature/cool")
      .expect("metadata recorded");
    assert_eq!(metadata.github_pr, Some(99));

    Ok(())
  }
}
