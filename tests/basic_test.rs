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
  assert!(
    stdout.contains("Git-based developer productivity tool"),
    "Help output not found"
  );
  assert!(stdout.contains("git"), "Git subcommand not found in help");
  assert!(stdout.contains("init"), "Init subcommand not found in help");
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
  assert!(
    stdout.contains("Git repository management"),
    "Git help output not found"
  );
  assert!(stdout.contains("add"), "Add subcommand not found in git help");
  assert!(stdout.contains("list"), "List subcommand not found in git help");
  assert!(stdout.contains("fetch"), "Fetch subcommand not found in git help");
}
