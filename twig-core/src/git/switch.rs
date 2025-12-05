//! Shared branch switching interface used by the CLI and Twig plugins.
//!
//! This module defines the request/response types and planning primitives for
//! future branch switching services. The goal is to centralise behaviour that
//! currently lives inside `twig-cli` so that the forthcoming `twig flow`
//! plugin can rely on the same implementation without duplicating logic or
//! tightly coupling to CLI-specific messaging.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use git2::{BranchType, Oid, Repository};
use regex::Regex;

use crate::git::checkout_branch;
use crate::git::graph::BranchName;
use crate::github::extract_pr_number_from_url;
use crate::jira_parser::JiraTicketParser;
use crate::output::print_info;
use crate::state::{BranchMetadata, RepoState};

static JIRA_ISSUE_URL_REGEX: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"/browse/([A-Z]{2,}-\d+)").expect("Failed to compile Jira issue URL regex"));

/// Input variants accepted by branch switching workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SwitchInput {
  /// Switch based on a Jira issue key (e.g., `PROJ-123`).
  JiraIssueKey(String),
  /// Switch based on a Jira issue URL.
  JiraIssueUrl(String),
  /// Switch based on a GitHub pull request id.
  GitHubPrId(u32),
  /// Switch based on a GitHub pull request URL.
  GitHubPrUrl(u32),
  /// Switch to a concrete branch name.
  BranchName(String),
}

/// Detect the switch input type (branch/Jira/PR) from raw user input.
pub fn detect_switch_input(jira_parser: Option<&JiraTicketParser>, input: &str) -> SwitchInput {
  // Check for GitHub PR URL
  if input.contains("github.com")
    && input.contains("/pull/")
    && let Ok(pr_number) = extract_pr_number_from_url(input)
  {
    return SwitchInput::GitHubPrUrl(pr_number);
  }

  // Check for Jira issue URL
  if (input.contains("atlassian.net/browse/") || (input.starts_with("http") && input.contains("/browse/")))
    && let Some(issue_key) = extract_jira_issue_from_url(input)
  {
    return SwitchInput::JiraIssueUrl(issue_key);
  }

  // Check for GitHub PR ID patterns (123, PR#123, #123)
  let cleaned_input = input.trim_start_matches("PR#").trim_start_matches('#');
  if let Ok(pr_number) = cleaned_input.parse::<u32>() {
    return SwitchInput::GitHubPrId(pr_number);
  }

  // Check for Jira issue key pattern (PROJ-123, ABC-456, etc.)
  if let Some(parser) = jira_parser
    && let Some(normalized_key) = parse_jira_issue_key(parser, input)
  {
    return SwitchInput::JiraIssueKey(normalized_key);
  }

  // Default to branch name
  SwitchInput::BranchName(input.to_string())
}

/// Options controlling how switch helpers behave.
#[derive(Debug, Clone, Default)]
pub struct SwitchExecutionOptions {
  /// Whether to create the target branch when it is missing.
  pub create_missing: bool,
  /// Parent selection hint when creating a new branch.
  pub parent_option: Option<String>,
}

/// Attempt to switch based on raw user input (branch/Jira/PR).
///
/// This helper combines input detection with local/remote checkout and basic
/// association storage so callers (CLI or plugins) do not have to duplicate the
/// branching logic from `twig switch`.
pub fn switch_from_input(
  repository: &Repository,
  repository_path: &Path,
  repo_state: &RepoState,
  jira_parser: Option<&JiraTicketParser>,
  raw_input: &str,
  options: &SwitchExecutionOptions,
) -> Result<BranchSwitchOutcome> {
  match detect_switch_input(jira_parser, raw_input) {
    SwitchInput::BranchName(name) => switch_to_branch_name(repository, repository_path, jira_parser, &name, options),
    SwitchInput::JiraIssueKey(key) | SwitchInput::JiraIssueUrl(key) => {
      switch_from_jira(repository, repository_path, repo_state, jira_parser, &key, options)
    }
    SwitchInput::GitHubPrId(pr) | SwitchInput::GitHubPrUrl(pr) => {
      switch_from_pr(repository, repository_path, repo_state, pr, options)
    }
  }
}

fn switch_to_branch_name(
  repository: &Repository,
  repository_path: &Path,
  jira_parser: Option<&JiraTicketParser>,
  branch_name: &str,
  options: &SwitchExecutionOptions,
) -> Result<BranchSwitchOutcome> {
  let target = BranchName::from(branch_name);
  let head = repository
    .head()
    .ok()
    .and_then(|h| h.shorthand().map(|s| s.to_string()));
  if head.as_deref() == Some(target.as_str()) {
    return Ok(BranchSwitchOutcome {
      branch: target.clone(),
      action: BranchSwitchAction::AlreadyCurrent,
      state_mutations: BranchStateMutations::default(),
    });
  }

  if branch_exists(repository, &target) {
    checkout_branch(repository, target.as_str())?;
    return Ok(BranchSwitchOutcome {
      branch: target,
      action: BranchSwitchAction::CheckedOutExisting,
      state_mutations: BranchStateMutations::default(),
    });
  }

  if !options.create_missing {
    return Err(anyhow::anyhow!(
      "Branch '{}' does not exist locally. Enable creation before switching.",
      branch_name
    ));
  }

  if try_checkout_remote_branch(repository, branch_name)? {
    return Ok(BranchSwitchOutcome {
      branch: BranchName::from(branch_name),
      action: BranchSwitchAction::CheckedOutRemote {
        remote: "origin".to_string(),
        remote_ref: BranchName::from(format!("origin/{branch_name}")),
      },
      state_mutations: BranchStateMutations::default(),
    });
  }

  let branch_base = resolve_branch_base(
    repository,
    repository_path,
    options.parent_option.as_deref(),
    jira_parser,
  )?;
  create_branch_from_base(repository, branch_name, branch_base)
}

fn switch_from_jira(
  repository: &Repository,
  repository_path: &Path,
  repo_state: &RepoState,
  jira_parser: Option<&JiraTicketParser>,
  issue_key: &str,
  options: &SwitchExecutionOptions,
) -> Result<BranchSwitchOutcome> {
  if let Some(branch_issue) = repo_state.get_branch_issue_by_jira(issue_key) {
    return switch_to_branch_name(repository, repository_path, jira_parser, &branch_issue.branch, options);
  }

  if !options.create_missing {
    return Err(anyhow::anyhow!(
      "No branch found for Jira issue {issue_key}. Enable creation to make one."
    ));
  }

  let branch_name = derive_jira_branch_name(issue_key);
  let mut outcome = switch_to_branch_name(repository, repository_path, jira_parser, &branch_name, options)?;
  outcome.state_mutations.issue = Some(IssueAssociation {
    key: issue_key.to_string(),
  });
  Ok(outcome)
}

fn switch_from_pr(
  repository: &Repository,
  repository_path: &Path,
  repo_state: &RepoState,
  pr_number: u32,
  options: &SwitchExecutionOptions,
) -> Result<BranchSwitchOutcome> {
  for branch_issue in repo_state.list_branch_issues() {
    if let Some(github_pr) = branch_issue.github_pr
      && github_pr == pr_number
    {
      return switch_to_branch_name(repository, repository_path, None, &branch_issue.branch, options);
    }
  }

  if !options.create_missing {
    return Err(anyhow::anyhow!(
      "No branch found for GitHub PR #{pr_number}. Enable creation to make one."
    ));
  }

  let branch_name = derive_pr_branch_name(pr_number);
  let mut outcome = switch_to_branch_name(repository, repository_path, None, &branch_name, options)?;
  outcome.state_mutations.github_pr = Some(pr_number);
  Ok(outcome)
}

fn derive_jira_branch_name(issue_key: &str) -> String {
  issue_key.to_lowercase()
}

fn derive_pr_branch_name(pr_number: u32) -> String {
  format!("pr/{pr_number}")
}

fn create_branch_from_base(
  repository: &Repository,
  branch_name: &str,
  branch_base: BranchBaseResolution,
) -> Result<BranchSwitchOutcome> {
  let commit = repository
    .find_commit(branch_base.commit())
    .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

  repository
    .branch(branch_name, &commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'"))?;
  checkout_branch(repository, branch_name)?;

  let mut state_mutations = BranchStateMutations::default();
  if let Some(parent) = branch_base.parent_name() {
    state_mutations.dependency = Some(BranchDependencyUpdate::Set(BranchParentReference::Branch(
      BranchName::from(parent),
    )));
  }

  let action = BranchSwitchAction::Created {
    base: BranchCreationBase {
      commit: commit.id(),
      source: branch_base.source(),
    },
    upstream: None,
  };

  Ok(BranchSwitchOutcome {
    branch: BranchName::from(branch_name),
    action,
    state_mutations,
  })
}

/// Parse and normalize a Jira issue key using the provided parser.
pub fn parse_jira_issue_key(parser: &JiraTicketParser, input: &str) -> Option<String> {
  parser.parse(input).ok()
}

/// Extract a Jira issue key from a Jira URL.
pub fn extract_jira_issue_from_url(url: &str) -> Option<String> {
  JIRA_ISSUE_URL_REGEX
    .captures(url)
    .and_then(|captures| captures.get(1))
    .map(|m| m.as_str().to_string())
}

/// Resolve the commit and parent metadata that should act as the base for a new
/// branch.
pub fn resolve_branch_base(
  repo: &Repository,
  repo_path: &Path,
  parent_option: Option<&str>,
  jira_parser: Option<&JiraTicketParser>,
) -> Result<BranchBaseResolution> {
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
        && let Some(normalized_key) = parse_jira_issue_key(parser, parent)
      {
        let repo_state = RepoState::load(repo_path)?;

        if let Some(branch_issue) = repo_state.get_branch_issue_by_jira(&normalized_key) {
          let commit =
            lookup_branch_tip(repo, &branch_issue.branch)?.ok_or_else(|| parent_lookup_error(&branch_issue.branch))?;
          return Ok(BranchBaseResolution::parent(branch_issue.branch.clone(), commit));
        }
      }

      let commit = lookup_branch_tip(repo, parent)?.ok_or_else(|| parent_lookup_error(parent))?;
      Ok(BranchBaseResolution::parent(parent.to_string(), commit))
    }
  }
}

/// Attempt to create a local tracking branch from a remote and checkout.
///
/// Returns `Ok(true)` when the remote branch was found and successfully
/// checked out, `Ok(false)` when the branch could not be located, and an
/// error for Git failures.
pub fn try_checkout_remote_branch(repo: &Repository, branch_name: &str) -> Result<bool> {
  let remote_branch_name = format!("origin/{branch_name}");
  let Some(commit_id) = lookup_branch_tip(repo, branch_name)? else {
    return Ok(false);
  };

  if repo.find_branch(&remote_branch_name, git2::BranchType::Remote).is_err() {
    return Ok(false);
  }

  let commit = repo
    .find_commit(commit_id)
    .with_context(|| format!("Failed to locate remote commit for '{remote_branch_name}'"))?;

  print_info(&format!(
    "Branch '{branch_name}' found on origin. Creating local tracking branch..."
  ));

  repo
    .branch(branch_name, &commit, false)
    .with_context(|| format!("Failed to create local branch '{branch_name}' from origin"))?;

  let mut local_branch = repo
    .find_branch(branch_name, git2::BranchType::Local)
    .with_context(|| format!("Failed to reopen branch '{branch_name}'"))?;
  local_branch
    .set_upstream(Some(&remote_branch_name))
    .with_context(|| format!("Failed to set upstream for '{branch_name}'"))?;

  drop(local_branch);

  checkout_branch(repo, branch_name)?;

  Ok(true)
}

/// Store Jira issue association in repository state.
pub fn store_jira_association(repo_path: &Path, branch_name: &str, issue_key: &str) -> Result<()> {
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

/// Store GitHub PR association in repository state.
pub fn store_github_pr_association(repo_path: &Path, branch_name: &str, pr_number: u32) -> Result<()> {
  let mut repo_state = RepoState::load(repo_path)?;

  let now = chrono::Utc::now().to_rfc3339();

  repo_state.add_branch_issue(BranchMetadata {
    branch: branch_name.to_string(),
    jira_issue: None,
    github_pr: Some(pr_number),
    created_at: now,
  });

  repo_state.save(repo_path)?;
  Ok(())
}

/// Persist requested state mutations after a branch switch.
pub fn apply_branch_state_mutations(repo_path: &Path, outcome: &BranchSwitchOutcome) -> Result<()> {
  if outcome.state_mutations.is_empty() {
    return Ok(());
  }

  let mut repo_state = RepoState::load(repo_path)?;

  if let Some(dependency) = &outcome.state_mutations.dependency {
    match dependency {
      BranchDependencyUpdate::Clear => {
        repo_state.remove_all_dependencies_for_branch(outcome.branch.as_str());
      }
      BranchDependencyUpdate::Set(parent) => match parent {
        BranchParentReference::Branch(name) => {
          repo_state.add_dependency(outcome.branch.to_string(), name.to_string())?;
        }
        BranchParentReference::IssueKey(_) => {}
      },
    }
  }

  if let Some(issue) = &outcome.state_mutations.issue {
    let now = chrono::Utc::now().to_rfc3339();
    repo_state.add_branch_issue(BranchMetadata {
      branch: outcome.branch.to_string(),
      jira_issue: Some(issue.key.clone()),
      github_pr: None,
      created_at: now,
    });
  }

  if let Some(pr) = outcome.state_mutations.github_pr {
    let now = chrono::Utc::now().to_rfc3339();
    repo_state.add_branch_issue(BranchMetadata {
      branch: outcome.branch.to_string(),
      jira_issue: None,
      github_pr: Some(pr),
      created_at: now,
    });
  }

  repo_state.save(repo_path)?;
  Ok(())
}

/// Resolve the tip commit for a branch, fetching origin if necessary.
pub fn lookup_branch_tip(repo: &Repository, branch_name: &str) -> Result<Option<Oid>> {
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
    if let Err(err) = remote.fetch(&[branch_name], Some(&mut fetch_options), None) {
      if err.code() == git2::ErrorCode::NotFound || err.class() == git2::ErrorClass::Reference {
        return Ok(None);
      }

      Err(err).with_context(|| format!("Failed to fetch '{branch_name}' from origin"))?;
    }

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

/// Source used when creating a new branch.
#[derive(Clone, Debug)]
pub enum BranchBase {
  /// Use the repository's current `HEAD` commit.
  Head,
  /// Use a specific parent branch name.
  Parent { name: String },
}

impl BranchBase {
  /// Return the parent branch name when applicable.
  pub fn parent_name(&self) -> Option<&str> {
    match self {
      BranchBase::Head => None,
      BranchBase::Parent { name } => Some(name.as_str()),
    }
  }
}

/// Resolved base for a new branch, including the commit to fork from.
#[derive(Clone, Debug)]
pub struct BranchBaseResolution {
  base: BranchBase,
  commit: git2::Oid,
}

impl BranchBaseResolution {
  /// Base the new branch on `HEAD`.
  pub fn head(commit: git2::Oid) -> Self {
    Self {
      base: BranchBase::Head,
      commit,
    }
  }

  /// Base the new branch on a parent branch.
  pub fn parent(name: String, commit: git2::Oid) -> Self {
    Self {
      base: BranchBase::Parent { name },
      commit,
    }
  }

  /// The commit to use when creating the branch.
  pub fn commit(&self) -> git2::Oid {
    self.commit
  }

  /// Optional parent branch name that should be linked to the new branch.
  pub fn parent_name(&self) -> Option<&str> {
    self.base.parent_name()
  }

  /// Map this resolution to a [`BranchBaseSource`] used for reporting.
  pub fn source(&self) -> BranchBaseSource {
    match &self.base {
      BranchBase::Head => BranchBaseSource::Head,
      BranchBase::Parent { name } => BranchBaseSource::LocalBranch(BranchName::from(name.as_str())),
    }
  }
}

/// Switch to the provided branch, creating it from `HEAD` when missing.
///
/// This helper mirrors the simple switching behaviour previously embedded in
/// the `twig flow` plugin so that CLI and plugin callers can share logic.
/// Callers are responsible for ensuring the repository is in a usable state
/// (non-bare, working tree present).
pub fn switch_or_create_local_branch(repository: &Repository, target: &BranchName) -> Result<BranchSwitchOutcome> {
  if branch_exists(repository, target) {
    checkout_branch(repository, target.as_str())?;

    return Ok(BranchSwitchOutcome {
      branch: target.clone(),
      action: BranchSwitchAction::CheckedOutExisting,
      state_mutations: BranchStateMutations::default(),
    });
  }

  let head_commit = repository
    .head()
    .context("Repository does not have an active HEAD commit")?
    .peel_to_commit()
    .context("Failed to resolve HEAD commit")?;

  repository
    .branch(target.as_str(), &head_commit, false)
    .with_context(|| format!("Failed to create branch \"{target}\" from HEAD"))?;
  checkout_branch(repository, target.as_str())?;

  Ok(BranchSwitchOutcome {
    branch: target.clone(),
    action: BranchSwitchAction::Created {
      base: BranchCreationBase {
        commit: head_commit.id(),
        source: BranchBaseSource::Head,
      },
      upstream: None,
    },
    state_mutations: BranchStateMutations::default(),
  })
}

fn branch_exists(repository: &Repository, target: &BranchName) -> bool {
  repository.find_branch(target.as_str(), BranchType::Local).is_ok()
}

/// Request describing a branch switch operation.
///
/// Callers construct this request after parsing CLI arguments or other user
/// input. Concrete services can then translate the high-level intent into a
/// sequence of Git operations and repository state updates.
#[derive(Debug, Clone)]
pub struct BranchSwitchRequest {
  /// Target that the user wants to end up on.
  pub target: BranchSwitchTarget,
  /// Whether the service may create new branches when the target is missing.
  pub creation_policy: BranchCreationPolicy,
  /// Preference for which branch (or commit) should act as the parent/base when
  /// creating a new branch.
  pub parent: BranchParentRequest,
  /// When true, the service should plan actions without mutating the
  /// repository.
  pub dry_run: bool,
}

impl BranchSwitchRequest {
  /// Convenience constructor for branch-name based requests that allow branch
  /// creation when the target does not already exist.
  pub fn for_branch(name: impl Into<BranchName>) -> Self {
    Self {
      target: BranchSwitchTarget::Branch(name.into()),
      creation_policy: BranchCreationPolicy::AllowCreate,
      parent: BranchParentRequest::Default,
      dry_run: false,
    }
  }
}

/// Target for a branch switch request.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchSwitchTarget {
  /// Switch to the root of the current branch's dependency tree.
  DependencyRoot,
  /// Switch to an explicit branch name.
  Branch(BranchName),
  /// Switch to (or create) a branch associated with an external issue (Jira,
  /// etc.).
  Issue(IssueReference),
  /// Switch to (or create) a branch associated with a GitHub pull request.
  GitHubPullRequest(GitHubPullRequestReference),
}

/// Reference to an external issue/work item used when creating or locating
/// associated branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueReference {
  /// Issue identifier (`PROJ-123`, `#42`, etc.).
  pub key: String,
  /// Optional human readable summary that can be used when generating branch
  /// names.
  pub summary: Option<String>,
}

impl IssueReference {
  /// Construct a reference for the provided tracker.
  pub fn new(key: impl Into<String>) -> Self {
    Self {
      key: key.into(),
      summary: None,
    }
  }

  /// Convenience helper for Jira references.
  pub fn jira(key: impl Into<String>) -> Self {
    Self::new(key)
  }
}

/// Reference to a GitHub pull request used when creating or locating branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestReference {
  /// Owning user or organisation for the repository.
  pub owner: Option<String>,
  /// Repository name.
  pub repository: Option<String>,
  /// Pull request sequence number.
  pub number: u32,
  /// Optional head information when already known (e.g. from a previous query).
  pub head: Option<PullRequestHead>,
}

impl GitHubPullRequestReference {
  /// Construct a GitHub pull request reference with minimal information.
  pub fn new(number: u32) -> Self {
    Self {
      owner: None,
      repository: None,
      number,
      head: None,
    }
  }
}

/// Description of a pull request head reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestHead {
  /// Branch name advertised by the pull request head.
  pub branch: BranchName,
  /// Optional remote name if it should be tracked explicitly.
  pub remote: Option<String>,
}

/// Policy controlling whether a missing branch may be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchCreationPolicy {
  /// Only allow switching when the branch already exists locally.
  RequireExisting,
  /// Permit the service to create the branch when it cannot be found.
  #[default]
  AllowCreate,
}

/// Preference for resolving the parent/base branch when creating a new branch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BranchParentRequest {
  /// Use the repository's current HEAD commit without linking dependencies.
  #[default]
  Default,
  /// Use the currently checked-out branch as the parent.
  CurrentBranch,
  /// Use an explicitly named branch.
  Explicit(BranchName),
  /// Resolve the parent branch by looking up the provided issue key.
  IssueKey(String),
  /// Do not link the new branch to any parent.
  None,
}

/// Environment required to plan a branch switch.
pub struct BranchSwitchContext<'repo> {
  /// Backing git2 repository handle.
  pub repository: &'repo Repository,
  /// Filesystem path to the repository root.
  pub repository_path: &'repo Path,
  /// Snapshot of Twig's repository state metadata.
  pub repo_state: &'repo RepoState,
}

/// Concrete actions that a branch switch service may perform.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchSwitchAction {
  /// The requested branch was already checked out.
  AlreadyCurrent,
  /// The branch existed and was checked out locally.
  CheckedOutExisting,
  /// The branch was fetched from a remote and checked out.
  CheckedOutRemote {
    /// Remote that supplied the branch contents.
    remote: String,
    /// Remote reference that was checked out (e.g. `origin/feature/foo`).
    remote_ref: BranchName,
  },
  /// A new local branch was created from the provided base.
  Created {
    /// Description of the base commit and branch used for the new branch.
    base: BranchCreationBase,
    /// Remote tracking branch configured for the new branch, when applicable.
    upstream: Option<String>,
  },
}

/// Description of the base commit used when creating or updating a branch.
#[derive(Debug, Clone)]
pub struct BranchCreationBase {
  /// Commit identifier used for the new branch tip.
  pub commit: Oid,
  /// Source reference that produced the commit.
  pub source: BranchBaseSource,
}

/// Source of a branch base resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BranchBaseSource {
  /// The repository's HEAD commit.
  Head,
  /// An existing local branch.
  LocalBranch(BranchName),
  /// A remote tracking branch.
  RemoteBranch {
    /// Remote name (`origin`, `upstream`, etc.).
    remote: String,
    /// Branch name advertised by the remote.
    branch: BranchName,
  },
}

/// Planned metadata updates that accompany a branch switch.
#[derive(Debug, Clone, Default)]
pub struct BranchStateMutations {
  /// Desired dependency relationship for the branch.
  pub dependency: Option<BranchDependencyUpdate>,
  /// Issue key to associate with the branch.
  pub issue: Option<IssueAssociation>,
  /// GitHub pull request number to associate with the branch.
  pub github_pr: Option<u32>,
}

impl BranchStateMutations {
  /// Determine whether the mutation set is empty.
  pub fn is_empty(&self) -> bool {
    self.dependency.is_none() && self.issue.is_none() && self.github_pr.is_none()
  }
}

/// Association between a branch and an external issue tracker item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueAssociation {
  /// Identifier within the tracker (currently Jira).
  pub key: String,
}

/// Desired dependency relationship for a branch.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchDependencyUpdate {
  /// Remove any existing dependency metadata.
  Clear,
  /// Set the dependency parent to the provided branch reference.
  Set(BranchParentReference),
}

/// Reference to a parent branch used when updating dependency metadata.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum BranchParentReference {
  /// Dependency pointing at an explicit branch name.
  Branch(BranchName),
  /// Dependency resolved via an issue tracker key.
  IssueKey(String),
}

/// Result returned by a branch switch service.
#[derive(Debug, Clone)]
pub struct BranchSwitchOutcome {
  /// Branch name that was ultimately checked out.
  pub branch: BranchName,
  /// Summary of the action that was taken.
  pub action: BranchSwitchAction,
  /// Metadata updates that should be applied after the switch completes.
  pub state_mutations: BranchStateMutations,
}

/// Trait implemented by branch switch planners.
///
/// The planner is responsible for translating high-level requests into a
/// concrete [`BranchSwitchOutcome`]. Implementations are expected to perform
/// necessary Git lookups and return rich metadata so that callers can present
/// consistent messaging and apply repository state updates.
pub trait BranchSwitchService {
  /// Plan a branch switch and return the resulting outcome.
  fn plan_switch(
    &mut self,
    context: BranchSwitchContext<'_>,
    request: BranchSwitchRequest,
  ) -> anyhow::Result<BranchSwitchOutcome>;
}

#[cfg(test)]
mod tests {
  use twig_test_utils::git::{GitRepoTestGuard, create_branch, create_commit};

  use super::*;
  use crate::jira_parser::{JiraParsingConfig, JiraTicketParser};

  #[test]
  fn detects_switch_inputs() {
    let parser = JiraTicketParser::new(JiraParsingConfig::default());

    assert_eq!(
      detect_switch_input(Some(&parser), "PROJ-123"),
      SwitchInput::JiraIssueKey("PROJ-123".to_string())
    );
    assert_eq!(
      detect_switch_input(Some(&parser), "https://company.atlassian.net/browse/PROJ-123"),
      SwitchInput::JiraIssueUrl("PROJ-123".to_string())
    );
    assert_eq!(
      detect_switch_input(Some(&parser), "https://github.com/owner/repo/pull/123"),
      SwitchInput::GitHubPrUrl(123)
    );
    assert_eq!(detect_switch_input(Some(&parser), "PR#42"), SwitchInput::GitHubPrId(42));
    assert_eq!(
      detect_switch_input(Some(&parser), "feature/branch"),
      SwitchInput::BranchName("feature/branch".to_string())
    );
  }

  #[test]
  fn parses_jira_key_and_urls() {
    let parser = JiraTicketParser::new(JiraParsingConfig::default());
    assert_eq!(parse_jira_issue_key(&parser, "PROJ-123"), Some("PROJ-123".to_string()));
    assert_eq!(
      extract_jira_issue_from_url("https://example.atlassian.net/browse/PROJ-999"),
      Some("PROJ-999".to_string())
    );
    assert!(extract_jira_issue_from_url("https://example.com/other").is_none());
  }

  #[test]
  fn extracts_pr_number_from_url() {
    assert_eq!(
      extract_pr_number_from_url("https://github.com/owner/repo/pull/123").unwrap(),
      123
    );
    assert!(extract_pr_number_from_url("https://github.com/owner/repo").is_err());
  }

  #[test]
  fn checks_out_existing_branch() -> Result<()> {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;
    create_branch(&guard.repo, "feature/existing", None)?;

    let outcome = switch_or_create_local_branch(&guard.repo, &BranchName::from("feature/existing"))?;

    assert!(matches!(outcome.action, BranchSwitchAction::CheckedOutExisting));

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/existing"));

    Ok(())
  }

  #[test]
  fn creates_branch_from_head_when_missing() -> Result<()> {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;

    let outcome = switch_or_create_local_branch(&guard.repo, &BranchName::from("feature/new"))?;

    match outcome.action {
      BranchSwitchAction::Created { base, upstream } => {
        assert_eq!(base.source, BranchBaseSource::Head);
        assert!(upstream.is_none());
      }
      action => panic!("unexpected action {action:?}"),
    }

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/new"));

    Ok(())
  }

  #[test]
  fn switches_using_jira_key_and_records_state() -> Result<()> {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;
    create_branch(&guard.repo, "feature/work", None)?;

    // Save association to repo state
    let repo_path = guard.repo.workdir().expect("workdir");
    let mut state = RepoState::load(repo_path)?;
    state.add_branch_issue(BranchMetadata {
      branch: "feature/work".into(),
      jira_issue: Some("PROJ-123".into()),
      github_pr: None,
      created_at: chrono::Utc::now().to_rfc3339(),
    });
    state.save(repo_path)?;

    let parser = JiraTicketParser::new(JiraParsingConfig::default());

    let options = SwitchExecutionOptions {
      create_missing: true,
      parent_option: None,
    };

    let outcome = switch_from_input(&guard.repo, repo_path, &state, Some(&parser), "PROJ-123", &options)
      .expect("switch should succeed");
    apply_branch_state_mutations(repo_path, &outcome)?;

    let refreshed = git2::Repository::open(repo_path)?;
    assert_eq!(refreshed.head()?.shorthand(), Some("feature/work"));

    Ok(())
  }

  #[test]
  fn creates_branch_for_jira_when_missing() -> Result<()> {
    let guard = GitRepoTestGuard::new();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;

    let repo_path = guard.repo.workdir().expect("workdir");
    let state = RepoState::load(repo_path)?;

    let parser = JiraTicketParser::new(JiraParsingConfig::default());

    let options = SwitchExecutionOptions {
      create_missing: true,
      parent_option: None,
    };

    let outcome = switch_from_input(&guard.repo, repo_path, &state, Some(&parser), "PROJ-999", &options)?;
    apply_branch_state_mutations(repo_path, &outcome)?;

    let refreshed = git2::Repository::open(repo_path)?;
    assert_eq!(refreshed.head()?.shorthand(), Some("proj-999"));

    let state_after = RepoState::load(repo_path)?;
    let metadata = state_after.get_branch_metadata("proj-999").expect("metadata stored");
    assert_eq!(metadata.jira_issue.as_deref(), Some("PROJ-999"));

    Ok(())
  }
}
