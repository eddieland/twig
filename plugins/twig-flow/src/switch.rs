use anyhow::Result;
use twig_core::git::switch::{BranchSwitchAction, switch_or_create_local_branch};
use twig_core::git::{BranchName, get_repository};
use twig_core::output::{print_error, print_success};

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

  match switch_or_create_local_branch(&repo, &BranchName::from(target.as_str())) {
    Ok(outcome) => match outcome.action {
      BranchSwitchAction::AlreadyCurrent | BranchSwitchAction::CheckedOutExisting => {
        print_success(&format!("Switched to branch \"{target}\"."));
      }
      BranchSwitchAction::Created { .. } => {
        print_success(&format!("Created and switched to new branch \"{target}\"."));
      }
      BranchSwitchAction::CheckedOutRemote { remote, remote_ref } => {
        print_success(&format!(
          "Checked out {remote_ref} from remote \"{remote}\" as \"{target}\"."
        ));
      }
      _ => {
        print_success(&format!("Switched to branch \"{target}\"."));
      }
    },
    Err(err) => {
      print_error(&format!("Failed to switch to {target}: {err}"));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::sync::Mutex;

  use twig_test_utils::{GitRepoTestGuard, checkout_branch as checkout, create_branch, create_commit};

  use super::*;

  static TEST_GUARD: Mutex<()> = Mutex::new(());

  #[test]
  fn switches_to_existing_branch() -> Result<()> {
    let _lock = TEST_GUARD.lock().unwrap();
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;
    create_branch(&guard.repo, "feature/existing", None)?;

    checkout(&guard.repo, "feature/existing")?;

    let cli = Cli {
      root: false,
      parent: false,
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
    let _lock = TEST_GUARD.lock().unwrap();
    let guard = GitRepoTestGuard::new_and_change_dir();
    create_commit(&guard.repo, "file.txt", "content", "initial")?;

    let cli = Cli {
      root: false,
      parent: false,
      target: Some("feature/new".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/new"));

    Ok(())
  }
}
