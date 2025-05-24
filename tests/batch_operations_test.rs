use std::process::Command;

#[test]
fn test_git_exec_help() {
    // This test verifies that the git exec help command works
    let output = Command::new("cargo")
        .args(["run", "--", "git", "exec", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed to execute successfully");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Execute a git command in repositories"),
        "Exec help output not found"
    );
    assert!(stdout.contains("--all"), "All flag not found in exec help");
    assert!(stdout.contains("--repo"), "Repo flag not found in exec help");
}

#[test]
fn test_git_stale_branches_help() {
    // This test verifies that the git stale-branches help command works
    let output = Command::new("cargo")
        .args(["run", "--", "git", "stale-branches", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command failed to execute successfully");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("List stale branches in repositories"),
        "Stale branches help output not found"
    );
    assert!(stdout.contains("--days"), "Days flag not found in stale-branches help");
    assert!(stdout.contains("--all"), "All flag not found in stale-branches help");
    assert!(stdout.contains("--repo"), "Repo flag not found in stale-branches help");
}

// Manual testing instructions:
// 1. Run `cargo build` to build the project
// 2. Test the git exec command:
//    - `./target/debug/twig git exec --repo . "git status"`
//    - `./target/debug/twig git exec --all "git status"`
// 3. Test the stale branches command:
//    - `./target/debug/twig git stale-branches --repo . --days 30`
//    - `./target/debug/twig git stale-branches --all --days 30`