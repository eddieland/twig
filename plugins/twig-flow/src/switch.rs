use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use directories::BaseDirs;
use git2::Repository;
use tokio::runtime::Runtime;
use twig_core::git::get_repository;
use twig_core::git::switch::{
  BranchSwitchAction, ParentBranchOption, SwitchExecutionOptions, SwitchInput, apply_branch_state_mutations,
  checkout_remote_branch, detect_switch_input, find_remote_branch, resolve_branch_base, store_jira_association,
  switch_from_input,
};
use twig_core::jira_parser::{JiraTicketParser, create_jira_parser};
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::state::RepoState;
use twig_core::{checkout_branch, generate_branch_name_from_issue, twig_theme};
use twig_jira::{create_jira_client_from_netrc, get_jira_host};

use crate::Cli;

/// Options when a Jira issue has no associated branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JiraBranchChoice {
  /// Create a branch by fetching issue details from Jira API.
  CreateFromJira,
  /// Create a simple branch using the lowercase issue key.
  CreateSimple,
  /// Let the user enter a custom branch name.
  CustomName,
  /// Abort the operation.
  Abort,
}

/// Options when a branch name doesn't exist locally or remotely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchCreateChoice {
  /// Create the branch with the given name.
  Create,
  /// Let the user enter a custom branch name.
  CustomName,
  /// Abort the operation.
  Abort,
}

/// Options when a remote tracking branch exists but no local branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteBranchChoice {
  /// Checkout the remote branch as a local tracking branch.
  CheckoutRemote,
  /// Create a new local branch (ignoring the remote).
  CreateLocal,
  /// Let the user enter a custom branch name.
  CustomName,
  /// Abort the operation.
  Abort,
}

/// Result of attempting to fetch a Jira issue for branch creation.
#[derive(Debug)]
enum JiraFetchOutcome {
  /// Successfully fetched the issue and generated a branch name.
  Success(String),
  /// The issue was not found in Jira (404).
  IssueNotFound,
  /// Jira is not configured or accessible (host, credentials, network, etc.).
  Unavailable,
}

/// Handle the branch switching mode for the `twig flow` plugin.
pub fn run(cli: &Cli) -> Result<()> {
  let Some(target) = cli.target.as_deref() else {
    return Ok(());
  };

  let target = target.to_string();

  let repo = match get_repository() {
    Some(repo) => repo,
    None => {
      print_error("Not in a git repository. Run this command from within a repository.");
      return Ok(());
    }
  };

  if repo.is_bare() {
    print_error("Cannot switch branches in a bare repository.");
    return Ok(());
  }

  let repo_path = match repo.workdir() {
    Some(path) => path,
    None => {
      print_error("Cannot switch branches in a bare repository.");
      return Ok(());
    }
  };

  let repo_state = RepoState::load(repo_path).unwrap_or_else(|_| RepoState::default());
  let jira_parser = create_jira_parser().or_else(|| Some(JiraTicketParser::new_default()));

  // Check if this is a Jira issue key without an existing association
  if let Some(issue_key) = detect_jira_without_association(&target, jira_parser.as_ref(), &repo_state) {
    return handle_jira_branch_creation(&repo, repo_path, &repo_state, jira_parser.as_ref(), &issue_key);
  }

  // Check if this is a branch name (not Jira/PR) for potential branch creation prompt
  let switch_input = detect_switch_input(jira_parser.as_ref(), &target);
  let is_branch_name_input = matches!(switch_input, SwitchInput::BranchName(_));

  // For branch name inputs, check if the branch exists locally first.
  // If it doesn't exist locally but exists on remote, prompt the user.
  if is_branch_name_input {
    let branch_exists_locally = repo.find_branch(&target, git2::BranchType::Local).is_ok();

    if !branch_exists_locally {
      // Check for remote branch
      if let Ok(Some(remote_branch)) = find_remote_branch(&repo, &target) {
        return handle_remote_branch(&repo, repo_path, jira_parser.as_ref(), &target, remote_branch.as_str());
      }
    }
  }

  // Standard switch flow for non-Jira inputs or Jira with existing association
  let options = SwitchExecutionOptions {
    create_missing: true,
    parent_option: ParentBranchOption::CurrentBranch,
  };

  match switch_from_input(&repo, repo_path, &repo_state, jira_parser.as_ref(), &target, &options) {
    Ok(outcome) => {
      if let Err(err) = apply_branch_state_mutations(repo_path, &outcome) {
        print_warning(&format!("Switched branches but failed to persist state: {err}"));
      }

      match outcome.action {
        BranchSwitchAction::AlreadyCurrent | BranchSwitchAction::CheckedOutExisting => {
          print_success(&format!("Switched to branch \"{}\".", outcome.branch));
        }
        BranchSwitchAction::Created { .. } => {
          print_success(&format!("Created and switched to new branch \"{}\".", outcome.branch));
        }
        BranchSwitchAction::CheckedOutRemote { remote, remote_ref } => {
          print_success(&format!(
            "Checked out {remote_ref} from remote \"{remote}\" as \"{}\".",
            outcome.branch
          ));
        }
        _ => {
          print_success(&format!("Switched to branch \"{}\".", outcome.branch));
        }
      }
    }
    Err(err) => {
      // If this was a branch name input and switching failed, offer to create it
      if is_branch_name_input {
        return handle_branch_creation(&repo, repo_path, jira_parser.as_ref(), &target, &err);
      }
      print_error(&format!("Failed to switch to {target}: {err}"));
    }
  }

  Ok(())
}

/// Check if the input is a Jira issue key without an existing branch
/// association.
fn detect_jira_without_association(
  input: &str,
  jira_parser: Option<&JiraTicketParser>,
  repo_state: &RepoState,
) -> Option<String> {
  match detect_switch_input(jira_parser, input) {
    SwitchInput::JiraIssueKey(key) | SwitchInput::JiraIssueUrl(key) => {
      // Check if there's already an associated branch
      if repo_state.get_branch_issue_by_jira(&key).is_some() {
        None // Has association, use normal flow
      } else {
        Some(key) // No association, needs user prompt
      }
    }
    _ => None, // Not a Jira issue
  }
}

/// Handle branch creation for a Jira issue that has no existing association.
fn handle_jira_branch_creation(
  repo: &Repository,
  repo_path: &Path,
  repo_state: &RepoState,
  jira_parser: Option<&JiraTicketParser>,
  issue_key: &str,
) -> Result<()> {
  let simple_name = issue_key.to_lowercase();

  // Check if we're in an interactive terminal
  if !std::io::stdin().is_terminal() {
    print_error(&format!(
      "No branch found for Jira issue {issue_key}. Cannot prompt for input in non-interactive mode."
    ));
    print_info("Hint: Use 'twig switch' to create a branch from Jira, or specify an existing branch name.");
    return Ok(());
  }

  // Try to fetch the Jira issue to show the actual branch name in the prompt
  let jira_branch_name = match try_fetch_jira_branch_name(issue_key) {
    JiraFetchOutcome::Success(name) => Some(name),
    JiraFetchOutcome::IssueNotFound => {
      print_warning(&format!(
        "Jira issue {issue_key} was not found. This may indicate a typo or Jira configuration issue."
      ));
      None
    }
    JiraFetchOutcome::Unavailable => None,
  };

  print_info(&format!(
    "No branch found for Jira issue {issue_key}. How would you like to proceed?"
  ));
  println!();

  let choice = prompt_jira_branch_choice(jira_branch_name.as_deref(), &simple_name)?;

  match choice {
    JiraBranchChoice::CreateFromJira => {
      if let Some(branch_name) = jira_branch_name {
        create_branch_with_name(repo, repo_path, jira_parser, issue_key, &branch_name)
      } else {
        // This shouldn't happen since we hide the option when fetch fails,
        // but handle it gracefully by fetching again
        create_branch_from_jira(repo, repo_path, jira_parser, issue_key)
      }
    }
    JiraBranchChoice::CreateSimple => {
      create_simple_branch(repo, repo_path, repo_state, jira_parser, issue_key, &simple_name)
    }
    JiraBranchChoice::CustomName => {
      let custom_name = prompt_custom_branch_name()?;
      if let Some(name) = custom_name {
        create_simple_branch(repo, repo_path, repo_state, jira_parser, issue_key, &name)
      } else {
        print_info("Aborted.");
        Ok(())
      }
    }
    JiraBranchChoice::Abort => {
      print_info("Aborted.");
      Ok(())
    }
  }
}

/// Prompt the user to choose how to create a branch for a Jira issue.
///
/// If `jira_branch_name` is Some, shows the actual branch name. If None (fetch failed),
/// the "Create branch from Jira" option is omitted.
fn prompt_jira_branch_choice(jira_branch_name: Option<&str>, simple_name: &str) -> Result<JiraBranchChoice> {
  let mut items = Vec::new();
  let mut choice_map = Vec::new();

  if let Some(branch_name) = jira_branch_name {
    items.push(format!("Create branch from Jira: {branch_name}"));
    choice_map.push(JiraBranchChoice::CreateFromJira);
  }

  items.push(format!("Create simple branch: {simple_name}"));
  choice_map.push(JiraBranchChoice::CreateSimple);

  items.push("Enter custom branch name".to_string());
  choice_map.push(JiraBranchChoice::CustomName);

  items.push("Abort".to_string());
  choice_map.push(JiraBranchChoice::Abort);

  let selection = dialoguer::Select::with_theme(&twig_theme())
    .with_prompt("Select an option")
    .items(&items)
    .default(0)
    .interact()
    .unwrap_or(choice_map.len() - 1); // Default to abort on error

  Ok(choice_map.get(selection).copied().unwrap_or(JiraBranchChoice::Abort))
}

/// Prompt the user to enter a custom branch name.
fn prompt_custom_branch_name() -> Result<Option<String>> {
  let input: String = dialoguer::Input::with_theme(&twig_theme())
    .with_prompt("Enter branch name (or leave empty to abort)")
    .allow_empty(true)
    .interact_text()
    .unwrap_or_default();

  let trimmed = input.trim();
  if trimmed.is_empty() {
    Ok(None)
  } else {
    Ok(Some(trimmed.to_string()))
  }
}

/// Handle branch creation when switching to a branch name that doesn't exist.
fn handle_branch_creation(
  repo: &Repository,
  repo_path: &Path,
  jira_parser: Option<&JiraTicketParser>,
  branch_name: &str,
  switch_error: &anyhow::Error,
) -> Result<()> {
  // Check if we're in an interactive terminal
  if !std::io::stdin().is_terminal() {
    print_error(&format!("Failed to switch to {branch_name}: {switch_error}"));
    return Ok(());
  }

  print_info(&format!(
    "Branch '{branch_name}' was not found locally or on origin. Would you like to create it?"
  ));
  println!();

  let choice = prompt_branch_create_choice(branch_name)?;

  match choice {
    BranchCreateChoice::Create => create_new_branch(repo, repo_path, jira_parser, branch_name),
    BranchCreateChoice::CustomName => {
      let custom_name = prompt_custom_branch_name()?;
      if let Some(name) = custom_name {
        create_new_branch(repo, repo_path, jira_parser, &name)
      } else {
        print_info("Aborted.");
        Ok(())
      }
    }
    BranchCreateChoice::Abort => {
      print_info("Aborted.");
      Ok(())
    }
  }
}

/// Prompt the user to choose how to create a branch.
fn prompt_branch_create_choice(branch_name: &str) -> Result<BranchCreateChoice> {
  let items = [
    format!("Create branch: {branch_name}"),
    "Enter custom branch name".to_string(),
    "Abort".to_string(),
  ];

  let choice_map = [
    BranchCreateChoice::Create,
    BranchCreateChoice::CustomName,
    BranchCreateChoice::Abort,
  ];

  let selection = dialoguer::Select::with_theme(&twig_theme())
    .with_prompt("Select an option")
    .items(&items)
    .default(0)
    .interact()
    .unwrap_or(choice_map.len() - 1); // Default to abort on error

  Ok(choice_map.get(selection).copied().unwrap_or(BranchCreateChoice::Abort))
}

/// Prompt the user to choose how to handle a remote branch.
fn prompt_remote_branch_choice(branch_name: &str, remote_branch: &str) -> Result<RemoteBranchChoice> {
  let items = [
    format!("Checkout remote branch: {remote_branch}"),
    format!("Create new local branch: {branch_name}"),
    "Enter custom branch name".to_string(),
    "Abort".to_string(),
  ];

  let choice_map = [
    RemoteBranchChoice::CheckoutRemote,
    RemoteBranchChoice::CreateLocal,
    RemoteBranchChoice::CustomName,
    RemoteBranchChoice::Abort,
  ];

  let selection = dialoguer::Select::with_theme(&twig_theme())
    .with_prompt("Select an option")
    .items(&items)
    .default(0)
    .interact()
    .unwrap_or(choice_map.len() - 1); // Default to abort on error

  Ok(choice_map.get(selection).copied().unwrap_or(RemoteBranchChoice::Abort))
}

/// Handle the case when a remote branch exists but no local branch.
fn handle_remote_branch(
  repo: &Repository,
  repo_path: &Path,
  jira_parser: Option<&JiraTicketParser>,
  branch_name: &str,
  remote_branch: &str,
) -> Result<()> {
  // Check if we're in an interactive terminal
  if !std::io::stdin().is_terminal() {
    print_error(&format!(
      "Branch '{branch_name}' exists on remote as '{remote_branch}' but not locally. \
       Cannot prompt for input in non-interactive mode."
    ));
    print_info("Hint: Use 'git checkout' to checkout the remote branch, or specify a different branch name.");
    return Ok(());
  }

  print_info(&format!(
    "Branch '{branch_name}' was found on remote as '{remote_branch}'. How would you like to proceed?"
  ));
  println!();

  let choice = prompt_remote_branch_choice(branch_name, remote_branch)?;

  match choice {
    RemoteBranchChoice::CheckoutRemote => {
      checkout_remote_branch(repo, branch_name, remote_branch)?;
      print_success(&format!(
        "Checked out '{remote_branch}' from remote as local branch '{branch_name}'."
      ));
    }
    RemoteBranchChoice::CreateLocal => {
      create_new_branch(repo, repo_path, jira_parser, branch_name)?;
    }
    RemoteBranchChoice::CustomName => {
      let custom_name = prompt_custom_branch_name()?;
      if let Some(name) = custom_name {
        create_new_branch(repo, repo_path, jira_parser, &name)?;
      } else {
        print_info("Aborted.");
      }
    }
    RemoteBranchChoice::Abort => {
      print_info("Aborted.");
    }
  }

  Ok(())
}

/// Create a new branch with the given name.
fn create_new_branch(
  repo: &Repository,
  repo_path: &Path,
  jira_parser: Option<&JiraTicketParser>,
  branch_name: &str,
) -> Result<()> {
  print_info(&format!("Creating branch: {branch_name}"));

  let branch_base = resolve_branch_base(repo, repo_path, &ParentBranchOption::CurrentBranch, jira_parser)?;

  let base_commit = repo
    .find_commit(branch_base.commit())
    .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

  repo
    .branch(branch_name, &base_commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'"))?;

  checkout_branch(repo, branch_name)?;

  if let Some(parent) = branch_base.parent_name() {
    let mut repo_state = RepoState::load(repo_path)?;
    repo_state.add_dependency(branch_name.to_string(), parent.to_string())?;
    repo_state.save(repo_path)?;
  }

  print_success(&format!("Created and switched to branch '{branch_name}'"));

  Ok(())
}

/// Attempt to fetch the Jira issue and generate the branch name.
/// Returns a structured outcome to distinguish between different failure modes.
fn try_fetch_jira_branch_name(issue_key: &str) -> JiraFetchOutcome {
  let jira_host = match get_jira_host() {
    Ok(host) => host,
    Err(_) => return JiraFetchOutcome::Unavailable,
  };

  let base_dirs = match BaseDirs::new() {
    Some(dirs) => dirs,
    None => return JiraFetchOutcome::Unavailable,
  };

  let jira_client = match create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host) {
    Ok(client) => client,
    Err(_) => return JiraFetchOutcome::Unavailable,
  };

  let rt = match Runtime::new() {
    Ok(rt) => rt,
    Err(_) => return JiraFetchOutcome::Unavailable,
  };

  rt.block_on(async {
    match jira_client.get_issue(issue_key).await {
      Ok(issue) => {
        let branch_name = generate_branch_name_from_issue(issue_key, &issue.fields.summary, false);
        JiraFetchOutcome::Success(branch_name)
      }
      Err(err) => {
        // Check if this is a "not found" error (404 from Jira API)
        let err_str = err.to_string();
        if err_str.contains("not found") {
          JiraFetchOutcome::IssueNotFound
        } else {
          JiraFetchOutcome::Unavailable
        }
      }
    }
  })
}

/// Create a branch with a pre-determined name (already fetched from Jira).
fn create_branch_with_name(
  repo: &Repository,
  repo_path: &Path,
  jira_parser: Option<&JiraTicketParser>,
  issue_key: &str,
  branch_name: &str,
) -> Result<()> {
  print_info(&format!("Creating branch: {branch_name}"));

  let branch_base = resolve_branch_base(repo, repo_path, &ParentBranchOption::CurrentBranch, jira_parser)?;

  let base_commit = repo
    .find_commit(branch_base.commit())
    .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

  repo
    .branch(branch_name, &base_commit, false)
    .with_context(|| format!("Failed to create branch '{branch_name}'"))?;

  checkout_branch(repo, branch_name)?;

  if let Some(parent) = branch_base.parent_name() {
    let mut repo_state = RepoState::load(repo_path)?;
    repo_state.add_dependency(branch_name.to_string(), parent.to_string())?;
    repo_state.save(repo_path)?;
  }

  store_jira_association(repo_path, branch_name, issue_key)?;

  print_success(&format!(
    "Created and switched to branch '{branch_name}' for Jira issue {issue_key}"
  ));

  Ok(())
}

/// Create a branch by fetching issue details from Jira API.
fn create_branch_from_jira(
  repo: &Repository,
  repo_path: &Path,
  jira_parser: Option<&JiraTicketParser>,
  issue_key: &str,
) -> Result<()> {
  let jira_host = match get_jira_host() {
    Ok(host) => host,
    Err(err) => {
      print_error(&format!("Failed to get Jira host: {err}"));
      print_info("Hint: Configure Jira in ~/.config/twig/jira.toml or set JIRA_HOST environment variable.");
      return Ok(());
    }
  };

  let base_dirs = match BaseDirs::new() {
    Some(dirs) => dirs,
    None => {
      print_error("Failed to determine home directory.");
      return Ok(());
    }
  };

  let jira_client = match create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host) {
    Ok(client) => client,
    Err(err) => {
      print_error(&format!("Failed to create Jira client: {err}"));
      print_info("Hint: Ensure Jira credentials are configured in ~/.netrc");
      return Ok(());
    }
  };

  let rt = Runtime::new().context("Failed to create async runtime")?;
  rt.block_on(async {
    let issue = match jira_client.get_issue(issue_key).await {
      Ok(issue) => issue,
      Err(err) => {
        print_error(&format!("Failed to fetch Jira issue {issue_key}: {err}"));
        return Ok(());
      }
    };

    // Create a branch name from the issue key and summary (without stop word filtering)
    let branch_name = generate_branch_name_from_issue(issue_key, &issue.fields.summary, false);

    print_info(&format!("Creating branch: {branch_name}"));

    // Resolve branch base from the current branch
    let branch_base = resolve_branch_base(repo, repo_path, &ParentBranchOption::CurrentBranch, jira_parser)?;

    // Create the branch
    let base_commit = repo
      .find_commit(branch_base.commit())
      .with_context(|| format!("Failed to locate base commit for '{branch_name}'"))?;

    repo
      .branch(&branch_name, &base_commit, false)
      .with_context(|| format!("Failed to create branch '{branch_name}'"))?;

    checkout_branch(repo, &branch_name)?;

    // Record parent dependency in repo state
    if let Some(parent) = branch_base.parent_name() {
      let mut repo_state = RepoState::load(repo_path)?;
      repo_state.add_dependency(branch_name.to_string(), parent.to_string())?;
      repo_state.save(repo_path)?;
    }

    // Store the Jira association
    store_jira_association(repo_path, &branch_name, issue_key)?;

    print_success(&format!(
      "Created and switched to branch '{branch_name}' for Jira issue {issue_key}"
    ));

    Ok(())
  })
}

/// Create a simple branch with the given name and associate it with the Jira
/// issue.
fn create_simple_branch(
  repo: &Repository,
  repo_path: &Path,
  repo_state: &RepoState,
  jira_parser: Option<&JiraTicketParser>,
  issue_key: &str,
  branch_name: &str,
) -> Result<()> {
  let options = SwitchExecutionOptions {
    create_missing: true,
    parent_option: ParentBranchOption::CurrentBranch,
  };

  // Use the standard switch flow but with our chosen branch name
  match switch_from_input(repo, repo_path, repo_state, jira_parser, branch_name, &options) {
    Ok(outcome) => {
      if let Err(err) = apply_branch_state_mutations(repo_path, &outcome) {
        print_warning(&format!("Switched branches but failed to persist state: {err}"));
      }

      // Store the Jira association if we created a new branch
      if matches!(outcome.action, BranchSwitchAction::Created { .. })
        && let Err(err) = store_jira_association(repo_path, branch_name, issue_key)
      {
        print_warning(&format!("Failed to store Jira association: {err}"));
      }

      print_success(&format!(
        "Created and switched to branch '{branch_name}' for Jira issue {issue_key}"
      ));
    }
    Err(err) => {
      print_error(&format!("Failed to create branch {branch_name}: {err}"));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use twig_core::state::{BranchMetadata, RepoState};
  use twig_test_utils::{GitRepoTestGuard, checkout_branch as checkout, create_branch, create_commit};

  use super::*;

  #[test]
  fn switches_to_existing_branch() -> Result<()> {
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;
    create_branch(&guard.repo, "feature/existing", None)?;

    checkout(&guard.repo, "feature/existing")?;

    let cli = Cli {
      root: false,
      parent: false,
      include: None,
      target: Some("feature/existing".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/existing"));

    Ok(())
  }

  #[test]
  fn creates_branch_when_missing() -> Result<()> {
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;

    let cli = Cli {
      root: false,
      parent: false,
      include: None,
      target: Some("feature/new".into()),
    };

    run(&cli)?;

    // Re-open the repository from the working directory to see the updated HEAD
    let repo_path = guard.repo.workdir().expect("workdir");
    let refreshed = git2::Repository::open(repo_path)?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/new"));

    // Verify the new branch is parented to the branch it was created from
    let state = RepoState::load(repo_path)?;
    let parents = state.get_dependency_parents("feature/new");
    assert_eq!(parents, vec!["main"]);

    Ok(())
  }

  #[test]
  fn switches_using_jira_association() -> Result<()> {
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;
    create_branch(&guard.repo, "feature/work", None)?;

    // Add Jira association
    let repo_path = guard.repo.workdir().expect("workdir");
    let mut state = RepoState::load(repo_path)?;
    state.add_branch_issue(BranchMetadata {
      branch: "feature/work".into(),
      jira_issue: Some("PROJ-123".into()),
      github_pr: None,
      created_at: "now".into(),
    });
    state.save(repo_path)?;

    let cli = Cli {
      root: false,
      parent: false,
      include: None,
      target: Some("PROJ-123".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    assert_eq!(refreshed.head()?.shorthand(), Some("feature/work"));

    Ok(())
  }

  #[test]
  fn detects_jira_without_association() {
    let parser = JiraTicketParser::new_default();
    let state = RepoState::default();

    // Should detect Jira issue without association
    let result = detect_jira_without_association("PROJ-123", Some(&parser), &state);
    assert_eq!(result, Some("PROJ-123".to_string()));

    // Should not detect branch names
    let result = detect_jira_without_association("feature/branch", Some(&parser), &state);
    assert_eq!(result, None);

    // Should not detect numbers (GitHub PR)
    let result = detect_jira_without_association("123", Some(&parser), &state);
    assert_eq!(result, None);
  }

  #[test]
  fn detects_jira_with_association() -> Result<()> {
    let parser = JiraTicketParser::new_default();
    let mut state = RepoState::default();
    state.add_branch_issue(BranchMetadata {
      branch: "existing-branch".into(),
      jira_issue: Some("PROJ-123".into()),
      github_pr: None,
      created_at: "now".into(),
    });

    // Should not detect because association exists
    let result = detect_jira_without_association("PROJ-123", Some(&parser), &state);
    assert_eq!(result, None);

    Ok(())
  }
}
