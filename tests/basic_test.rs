use std::process::Command;

#[test]
fn test_version_command() {
  // This test verifies that the binary can be built and run with the version command
  let output = Command::new("cargo")
    .args(["run", "--", "version"])
    .output()
    .expect("Failed to execute command");

  assert!(output.status.success(), "Command failed to execute successfully");

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("twig version"), "Version output not found");
}

#[test]
fn test_placeholder() {
  // This is a simple placeholder test that always passes
  assert!(true, "This test should always pass");
}
