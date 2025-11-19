use anyhow::{Context, Result};
use git2::{BranchType, Repository};
use twig_core::git::{checkout_branch, get_repository};
use twig_core::output::{print_error, print_success};

use crate::Cli;

enum SwitchOutcome {
  CheckedOut,
  Created,
}

/// Handle the branch switching mode for the `twig flow` plugin.
pub fn run(cli: &Cli) -> Result<()> {
  let Some(target) = cli.target.as_deref() else {
    return Ok(());
  };

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

  match switch_branch(&repo, target) {
    Ok(SwitchOutcome::CheckedOut) => {
      print_success(&format!("Switched to branch \"{target}\"."));
    }
    Ok(SwitchOutcome::Created) => {
      print_success(&format!("Created and switched to new branch \"{target}\"."));
    }
    Err(err) => {
      print_error(&format!("Failed to switch to {target}: {err}"));
    }
  }

  Ok(())
}

fn switch_branch(repo: &Repository, target: &str) -> Result<SwitchOutcome> {
  if branch_exists(repo, target) {
    checkout_branch(repo, target).with_context(|| format!("Failed to checkout {target}"))?;
    return Ok(SwitchOutcome::CheckedOut);
  }

  let head_commit = repo
    .head()
    .context("Repository does not have an active HEAD commit")?
    .peel_to_commit()
    .context("Failed to resolve HEAD commit")?;

  repo
    .branch(target, &head_commit, false)
    .with_context(|| format!("Failed to create branch \"{target}\" from HEAD"))?;
  checkout_branch(repo, target).with_context(|| format!("Failed to checkout {target}"))?;

  Ok(SwitchOutcome::Created)
}

fn branch_exists(repo: &Repository, name: &str) -> bool {
  repo.find_branch(name, BranchType::Local).is_ok()
}

#[cfg(test)]
mod tests {
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
      target: Some("feature/new".into()),
    };

    run(&cli)?;

    let refreshed = git2::Repository::open(guard.repo.path())?;
    let head = refreshed.head()?;
    assert_eq!(head.shorthand(), Some("feature/new"));

    Ok(())
  }
}
