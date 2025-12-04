use anyhow::Result;
use twig_core::git::get_repository;
use twig_core::git::switch::{
  BranchSwitchAction, SwitchExecutionOptions, apply_branch_state_mutations, switch_from_input,
};
use twig_core::jira_parser::{JiraTicketParser, create_jira_parser};
use twig_core::output::{print_error, print_success, print_warning};
use twig_core::state::RepoState;

use crate::Cli;

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

  let options = SwitchExecutionOptions {
    create_missing: true,
    parent_option: None,
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
      print_error(&format!("Failed to switch to {target}: {err}"));
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
      filter: None,
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
      filter: None,
      target: Some("feature/new".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/new"));

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
      filter: None,
      target: Some("PROJ-123".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    assert_eq!(refreshed.head()?.shorthand(), Some("feature/work"));

    Ok(())
  }

  #[test]
  fn creates_branch_for_jira_input() -> Result<()> {
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;

    let cli = Cli {
      root: false,
      parent: false,
      filter: None,
      target: Some("PROJ-999".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    assert_eq!(refreshed.head()?.shorthand(), Some("proj-999"));

    let repo_path = guard.repo.workdir().expect("workdir");
    let state = RepoState::load(repo_path)?;
    let metadata = state.get_branch_metadata("proj-999").expect("metadata stored");
    assert_eq!(metadata.jira_issue.as_deref(), Some("PROJ-999"));

    Ok(())
  }
}
