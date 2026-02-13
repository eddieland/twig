use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use twig_test_utils::{GitRepoTestGuard, create_commit};

#[test]
fn help_output_shows_usage() {
  cargo_bin_cmd!("twig-prune")
    .arg("--help")
    .assert()
    .success()
    .stdout(predicate::str::contains("Delete local branches whose GitHub PRs have been merged"))
    .stdout(predicate::str::contains("--yes-i-really-want-to-skip-prompts"))
    .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn skip_prompts_flag_is_documented() {
  cargo_bin_cmd!("twig-prune")
    .arg("--help")
    .assert()
    .success()
    .stdout(predicate::str::contains("Delete without prompting"));
}

#[test]
fn errors_when_no_origin_remote() {
  let guard = GitRepoTestGuard::new();
  create_commit(&guard.repo, "README.md", "hello", "initial commit").unwrap();

  cargo_bin_cmd!("twig-prune")
    .current_dir(guard.path())
    .assert()
    .failure()
    .stderr(predicate::str::contains("origin"));
}
