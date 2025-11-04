//! # Git Utilities
//!
//! Provides Git repository detection, branch operations, and other Git-related
//! utilities for plugins to interact with Git repositories.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{self, BranchType, ErrorClass, ErrorCode, FetchOptions, Oid, Repository};

use crate::jira_parser::JiraTicketParser;
use crate::output::{print_info, print_success, print_warning};
use crate::state::RepoState;

/// Detect if the current directory or any parent directory is a Git repository
pub fn detect_repository() -> Option<PathBuf> {
  let current_dir = env::current_dir().ok()?;
  detect_repository_from_path(&current_dir)
}

/// Detect if the given path or any parent directory is a Git repository
pub fn detect_repository_from_path<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
  let path = path.as_ref();

  match Repository::discover(path) {
    Ok(repo) => repo.workdir().map(|workdir| workdir.to_path_buf()),
    Err(_) => None,
  }
}

/// Get the current branch name if we're in a Git repository
pub fn current_branch() -> Result<Option<String>> {
  let repo_path = detect_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  let repo = Repository::open(&repo_path).context("Failed to open Git repository")?;

  let head = repo.head().context("Failed to get HEAD reference")?;

  if let Some(branch_name) = head.shorthand() {
    Ok(Some(branch_name.to_string()))
  } else {
    Ok(None)
  }
}

/// Check if we're currently in a git repository
pub fn in_git_repository() -> bool {
  detect_repository().is_some()
}

/// Get the Git repository object for the current directory
pub fn get_repository() -> Option<Repository> {
  let repo_path = detect_repository()?;

  let repo = Repository::open(&repo_path)
    .context("Failed to open Git repository")
    .ok()?;

  Some(repo)
}

/// Get the Git repository object for a specific path
pub fn get_repository_from_path<P: AsRef<Path>>(path: P) -> Option<Repository> {
  let repo_path = detect_repository_from_path(path)?;

  let repo = Repository::open(&repo_path)
    .context("Failed to open Git repository")
    .ok()?;

  Some(repo)
}

/// Check if a branch exists in the repository
pub fn branch_exists(branch_name: &str) -> Result<bool> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(_) => Ok(true),
    Err(_) => Ok(false),
  }
}

/// Get all local branches in the repository
pub fn get_local_branches() -> Result<Vec<String>> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  let branches = repo.branches(Some(git2::BranchType::Local))?;
  let mut branch_names = Vec::new();

  for branch_result in branches {
    let (branch, _) = branch_result?;
    if let Some(name) = branch.name()? {
      branch_names.push(name.to_string());
    }
  }

  Ok(branch_names)
}

/// Get the remote tracking branch for a local branch
pub fn get_upstream_branch(branch_name: &str) -> Result<Option<String>> {
  let repo = get_repository().ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?;

  let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
    Ok(branch) => branch,
    Err(_) => return Ok(None),
  };

  match branch.upstream() {
    Ok(upstream) => {
      if let Some(name) = upstream.name()? {
        Ok(Some(name.to_string()))
      } else {
        Ok(None)
      }
    }
    Err(_) => Ok(None),
  }
}

/// Checkout an existing local branch using an already-opened repository handle.
pub fn checkout_branch_with_repo(repo: &Repository, branch_name: &str) -> Result<()> {
  let branch = repo
    .find_branch(branch_name, git2::BranchType::Local)
    .with_context(|| format!("Branch '{branch_name}' not found"))?;

  let target = branch
    .get()
    .target()
    .ok_or_else(|| anyhow::anyhow!("Branch '{branch_name}' has no target commit"))?;

  repo
    .set_head(&format!("refs/heads/{branch_name}"))
    .with_context(|| format!("Failed to set HEAD to branch '{branch_name}'"))?;

  let object = repo.find_object(target, None)?;
  let mut builder = git2::build::CheckoutBuilder::new();

  repo
    .checkout_tree(&object, Some(&mut builder))
    .with_context(|| format!("Failed to checkout branch '{branch_name}'"))?;

  Ok(())
}

/// Checkout an existing local branch within the provided repository path.
pub fn checkout_branch<P: AsRef<Path>>(repo_path: P, branch_name: &str) -> Result<()> {
  let repo = Repository::open(repo_path.as_ref()).context("Failed to open Git repository")?;
  checkout_branch_with_repo(&repo, branch_name)
}

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
pub struct BranchBaseResolution {
  base: BranchBase,
  commit: Oid,
}

impl BranchBaseResolution {
  fn head(commit: Oid) -> Self {
    Self {
      base: BranchBase::Head,
      commit,
    }
  }

  fn parent(name: String, commit: Oid) -> Self {
    Self {
      base: BranchBase::Parent { name },
      commit,
    }
  }

  pub fn parent_name(&self) -> Option<&str> {
    self.base.parent_name()
  }

  pub fn commit(&self) -> Oid {
    self.commit
  }
}

pub fn resolve_branch_base_with_repo(
  repo: &Repository,
  parent_option: Option<&str>,
  jira_parser: Option<&JiraTicketParser>,
  repo_state: Option<&RepoState>,
) -> Result<BranchBaseResolution> {
  let mut repo_state_owned: Option<RepoState> = None;
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
      if let Some(parser) = jira_parser
        && let Ok(normalized_key) = parser.parse(parent)
      {
        let repo_state_ref = match repo_state {
          Some(state) => state,
          None => {
            let repo_path = repo
              .workdir()
              .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))?;
            repo_state_owned = Some(RepoState::load(repo_path)?);
            repo_state_owned.as_ref().unwrap()
          }
        };

        if let Some(branch_issue) = repo_state_ref.get_branch_issue_by_jira(&normalized_key) {
          let commit =
            lookup_branch_tip(&repo, &branch_issue.branch)?.ok_or_else(|| parent_lookup_error(&branch_issue.branch))?;
          return Ok(BranchBaseResolution::parent(branch_issue.branch.clone(), commit));
        }
      }

      let commit = lookup_branch_tip(&repo, parent)?.ok_or_else(|| parent_lookup_error(parent))?;
      Ok(BranchBaseResolution::parent(parent.to_string(), commit))
    }
  }
}

pub fn resolve_branch_base(
  repo_path: &Path,
  parent_option: Option<&str>,
  jira_parser: Option<&JiraTicketParser>,
) -> Result<BranchBaseResolution> {
  let repo = Repository::open(repo_path)?;
  resolve_branch_base_with_repo(&repo, parent_option, jira_parser, None)
}

pub fn try_checkout_remote_branch_with_repo(repo: &Repository, branch_name: &str) -> Result<bool> {
  let remote_branch_name = format!("origin/{branch_name}");
  let Some(commit_id) = lookup_branch_tip(repo, branch_name)? else {
    return Ok(false);
  };

  if repo.find_branch(&remote_branch_name, BranchType::Remote).is_err() {
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
    .find_branch(branch_name, BranchType::Local)
    .with_context(|| format!("Failed to reopen branch '{branch_name}'"))?;
  local_branch
    .set_upstream(Some(&remote_branch_name))
    .with_context(|| format!("Failed to set upstream for '{branch_name}'"))?;

  drop(local_branch);

  switch_to_branch_with_repo(repo, branch_name)?;

  Ok(true)
}

pub fn try_checkout_remote_branch<P: AsRef<Path>>(repo_path: P, branch_name: &str) -> Result<bool> {
  let repo = Repository::open(repo_path.as_ref())?;
  try_checkout_remote_branch_with_repo(&repo, branch_name)
}

pub fn switch_to_branch_with_repo(repo: &Repository, branch_name: &str) -> Result<()> {
  checkout_branch_with_repo(repo, branch_name)?;
  print_success(&format!("Switched to branch '{branch_name}'"));
  Ok(())
}

pub fn switch_to_branch<P: AsRef<Path>>(repo_path: P, branch_name: &str) -> Result<()> {
  let repo = Repository::open(repo_path.as_ref())?;
  switch_to_branch_with_repo(&repo, branch_name)
}

pub fn create_and_switch_to_branch<P: AsRef<Path>>(
  repo_path: P,
  branch_name: &str,
  branch_base: &BranchBaseResolution,
) -> Result<()> {
  let repo_path = repo_path.as_ref();
  let repo = Repository::open(repo_path)?;

  create_and_switch_to_branch_with_repo(&repo, branch_name, branch_base, None)
}

pub fn create_and_switch_to_branch_with_repo(
  repo: &Repository,
  branch_name: &str,
  branch_base: &BranchBaseResolution,
  mut repo_state: Option<&mut RepoState>,
) -> Result<()> {
  let base_commit = repo
    .find_commit(branch_base.commit())
    .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

  repo
    .branch(branch_name, &base_commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'"))?;

  print_success(&format!("Created branch '{branch_name}'"));

  switch_to_branch_with_repo(repo, branch_name)?;

  if let Some(parent) = branch_base.parent_name() {
    add_branch_dependency(repo, repo_state.as_deref_mut(), branch_name, parent)?;
  }

  Ok(())
}

fn add_branch_dependency(
  repo: &Repository,
  repo_state: Option<&mut RepoState>,
  child: &str,
  parent: &str,
) -> Result<()> {
  let repo_path = repo
    .workdir()
    .ok_or_else(|| anyhow::anyhow!("Repository has no working directory"))?;

  let mut owned_state: Option<RepoState> = None;
  let state: &mut RepoState = match repo_state {
    Some(state) => state,
    None => {
      owned_state = Some(RepoState::load(repo_path)?);
      owned_state
        .as_mut()
        .expect("owned_state just initialized with RepoState::load")
    }
  };

  match state.add_dependency(child.to_string(), parent.to_string()) {
    Ok(()) => {
      state.save(repo_path)?;
      print_success(&format!("Added dependency: {child} -> {parent}"));
      Ok(())
    }
    Err(e) => {
      print_warning(&format!("Failed to add dependency: {e}"));
      Ok(())
    }
  }
}

fn lookup_branch_tip(repo: &Repository, branch_name: &str) -> Result<Option<Oid>> {
  if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
    let commit = branch
      .into_reference()
      .peel_to_commit()
      .context("Failed to resolve local branch commit")?;
    return Ok(Some(commit.id()));
  }

  let remote_branch_name = format!("origin/{branch_name}");
  if let Ok(branch) = repo.find_branch(&remote_branch_name, BranchType::Remote) {
    let commit = branch
      .into_reference()
      .peel_to_commit()
      .context("Failed to resolve remote branch commit")?;
    return Ok(Some(commit.id()));
  }

  if let Ok(mut remote) = repo.find_remote("origin") {
    let mut fetch_options = FetchOptions::new();
    if let Err(err) = remote.fetch(&[branch_name], Some(&mut fetch_options), None) {
      if err.code() == ErrorCode::NotFound || err.class() == ErrorClass::Reference {
        return Ok(None);
      }

      Err(err).with_context(|| format!("Failed to fetch '{branch_name}' from origin"))?;
    }

    if let Ok(branch) = repo.find_branch(&remote_branch_name, BranchType::Remote) {
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

#[cfg(test)]
mod tests {
  use anyhow::Result;
  use tempfile::TempDir;
  use twig_test_utils::{
    GitRepoTestGuard, checkout_branch as utils_checkout_branch, create_commit, setup_test_env_with_init,
  };

  use super::*;

  #[test]
  fn test_detect_repository_none() {
    let temp_dir = TempDir::new().unwrap();
    let result = detect_repository_from_path(temp_dir.path());
    assert!(result.is_none());
  }

  #[test]
  fn test_detect_repository_exists() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    Repository::init(repo_path).unwrap();

    let maybe_result = detect_repository_from_path(repo_path);
    assert!(maybe_result.is_some());

    let result = maybe_result.unwrap();
    assert_eq!(
      std::fs::canonicalize(result).unwrap(),
      std::fs::canonicalize(repo_path).unwrap()
    );
  }

  #[test]
  fn test_in_git_repository() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Test non-git directory
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(repo_path).unwrap();
    assert!(!in_git_repository());

    // Initialize git repository and test again
    Repository::init(repo_path).unwrap();
    assert!(in_git_repository());

    // Restore original directory
    env::set_current_dir(original_dir).unwrap();
  }

  #[test]
  fn test_get_local_branches() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    let repo = Repository::init(repo_path).unwrap();

    // Create an initial commit to establish main branch
    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
      let mut index = repo.index().unwrap();
      index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    repo
      .commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
      .unwrap();

    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(repo_path).unwrap();

    let branches = get_local_branches().unwrap();
    assert!(!branches.is_empty());

    // Restore original directory
    env::set_current_dir(original_dir).unwrap();
  }

  #[test]
  fn test_checkout_branch() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    let repo = Repository::init(repo_path).unwrap();

    let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
      let mut index = repo.index().unwrap();
      index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();

    repo
      .commit(Some("HEAD"), &signature, &signature, "Initial commit", &tree, &[])
      .unwrap();

    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/test", &head_commit, false).unwrap();

    checkout_branch(repo_path, "feature/test").unwrap();

    let repo = Repository::open(repo_path).unwrap();
    let head = repo.head().unwrap();
    assert_eq!(head.shorthand(), Some("feature/test"));
  }

  #[test]
  fn test_resolve_branch_base_with_parent_branch() -> Result<()> {
    let (_env_guard, _config_dirs) = setup_test_env_with_init()?;
    let repo_guard = GitRepoTestGuard::new();
    let repo = &repo_guard.repo;

    create_commit(repo, "base.txt", "base", "initial commit")?;
    let head_commit = repo.head()?.peel_to_commit()?;
    repo.branch("parent", &head_commit, true)?;
    utils_checkout_branch(repo, "parent")?;
    create_commit(repo, "parent.txt", "parent", "parent commit")?;

    let branch_base = resolve_branch_base(repo_guard.path(), Some("parent"), None)?;
    assert_eq!(branch_base.parent_name(), Some("parent"));

    let repo_state = RepoState::load(repo_guard.path())?;
    assert!(repo_state.dependencies.is_empty());

    Ok(())
  }

  #[test]
  fn test_try_checkout_remote_branch_creates_local_tracking_branch() -> Result<()> {
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

    assert!(try_checkout_remote_branch(repo_guard.path(), "feature/existing")?);

    let repo = Repository::open(repo_guard.path())?;
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
}
