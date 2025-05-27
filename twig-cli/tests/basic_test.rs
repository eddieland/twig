use std::process::Command;

#[test]
fn test_help_command() {
  // This test verifies that the help command works
  let output = Command::new("cargo")
    .args(["run", "--", "--help"])
    .output()
    .expect("Failed to execute command");

  assert!(output.status.success(), "Command failed to execute successfully");

  let stdout = String::from_utf8_lossy(&output.stdout);
  // Check for presence of main commands rather than specific text
  assert!(stdout.contains("twig"), "Main command not found in help output");
  assert!(stdout.contains("git"), "Git subcommand not found in help");
  assert!(stdout.contains("init"), "Init subcommand not found in help");
  assert!(stdout.contains("worktree"), "Worktree subcommand not found in help");
}

#[test]
fn test_git_help_command() {
  // This test verifies that the git help command works
  let output = Command::new("cargo")
    .args(["run", "--", "git", "--help"])
    .output()
    .expect("Failed to execute command");

  assert!(output.status.success(), "Command failed to execute successfully");

  let stdout = String::from_utf8_lossy(&output.stdout);
  // Check for presence of git subcommands rather than specific text
  assert!(stdout.contains("git"), "Git command not found in help output");
  assert!(stdout.contains("add"), "Add subcommand not found in git help");
  assert!(stdout.contains("remove"), "Remove subcommand not found in git help");
  assert!(stdout.contains("list"), "List subcommand not found in git help");
  assert!(stdout.contains("fetch"), "Fetch subcommand not found in git help");
  assert!(stdout.contains("exec"), "Exec subcommand not found in git help");
  assert!(
    stdout.contains("stale-branches"),
    "Stale-branches subcommand not found in git help"
  );
}
