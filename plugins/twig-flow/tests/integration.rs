use std::str;

use anyhow::Result;
use assert_cmd::cargo::cargo_bin_cmd;
use git2::Repository;
use predicates::prelude::*;
use twig_core::state::RepoState;
use twig_test_utils::{GitRepoTestGuard, checkout_branch, create_branch, create_commit};

#[test]
fn renders_branch_tree_output() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  create_commit(&guard.repo, "README.md", "hello", "initial commit")?;
  create_branch(&guard.repo, "feature/login", None)?;
  create_branch(&guard.repo, "feature/payment", None)?;

  let main_branch = guard.repo.head()?.shorthand().unwrap().to_string();

  checkout_branch(&guard.repo, "feature/login")?;
  create_commit(&guard.repo, "login.txt", "wip", "login work")?;

  checkout_branch(&guard.repo, &main_branch)?;
  create_commit(&guard.repo, "main.txt", "progress", "main work")?;

  let head = guard.repo.head()?.shorthand().unwrap().to_string();
  let mut state = RepoState::default();
  state.add_root(head.clone(), true)?;
  state.add_dependency("feature/login".into(), head.clone())?;
  state.add_dependency("feature/payment".into(), head.clone())?;
  state.save(guard.path())?;

  let assert = cargo_bin_cmd!("twig-flow")
    .env("NO_COLOR", "1")
    .current_dir(guard.path())
    .assert()
    .success();

  let stdout = str::from_utf8(&assert.get_output().stdout)?;
  assert!(stdout.contains("Branch"));
  assert!(stdout.contains("feature/login"));
  assert!(stdout.contains("feature/payment"));
  assert!(stdout.contains("feature/login (+1/-1)"));

  Ok(())
}

#[test]
fn errors_when_no_root_branch_configured() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  create_commit(&guard.repo, "README.md", "hello", "initial commit")?;

  // No root branch configured — twig flow should error
  cargo_bin_cmd!("twig-flow")
    .env("NO_COLOR", "1")
    .current_dir(guard.path())
    .assert()
    .success()
    .stderr(predicate::str::contains("No root branches configured"));

  Ok(())
}

#[test]
fn creates_and_switches_to_missing_branch() -> Result<()> {
  let guard = GitRepoTestGuard::new();
  create_commit(&guard.repo, "README.md", "hello", "initial commit")?;

  cargo_bin_cmd!("twig-flow")
    .env("NO_COLOR", "1")
    .current_dir(guard.path())
    .arg("feature/new-flow-branch")
    .assert()
    .success()
    .stdout(predicate::str::contains("feature/new-flow-branch"));

  let refreshed = Repository::open(guard.path())?;
  let head = refreshed.head()?;
  assert_eq!(head.shorthand(), Some("feature/new-flow-branch"));

  Ok(())
}
