//! # Git Command Helpers
//!
//! Shared utilities for spawning git sub-processes and capturing their output.
//! Both the `cascade` and `rebase` commands rely on these helpers so that the
//! implementation stays in sync.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::consts;

/// Output from a git command, including both the combined stdout/stderr text
/// and whether the process exited successfully (exit code 0).
pub struct GitCommandOutput {
  /// Combined stdout and stderr text.
  pub output: String,
  /// Whether the command exited with status code 0.
  pub success: bool,
}

/// Execute a git command and return its combined output along with the exit
/// status.
pub fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<GitCommandOutput> {
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(args)
    .current_dir(repo_path)
    .output()
    .context(format!("Failed to execute git command: {args:?}"))?;

  let success = output.status.success();
  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();

  let mut combined = String::new();
  if !stdout.is_empty() {
    combined.push_str(&stdout);
  }
  if !stderr.is_empty() {
    if !combined.is_empty() {
      combined.push('\n');
    }
    combined.push_str(&stderr);
  }

  Ok(GitCommandOutput {
    output: combined,
    success,
  })
}

/// Execute a git command with inherited stdin/stdout/stderr.
///
/// Used for commands like `rebase --continue` / `--skip` that may need to
/// open an editor or interact with the user.
///
/// # Returns
///
/// A [`GitCommandOutput`] where `output` is always empty (inherited I/O means
/// there is nothing to capture) and `success` reflects the process exit code.
pub fn execute_git_command_interactive(repo_path: &Path, args: &[&str]) -> Result<GitCommandOutput> {
  let status = Command::new(consts::GIT_EXECUTABLE)
    .args(args)
    .current_dir(repo_path)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .status()
    .context(format!("Failed to execute git command: {args:?}"))?;

  Ok(GitCommandOutput {
    output: String::new(),
    success: status.success(),
  })
}

/// Resolve the upstream remote name configured for `branch`.
///
/// Reads `branch.<name>.remote` from git config. Returns `None` when no
/// remote is configured (e.g. the branch has never been pushed).
pub fn resolve_branch_remote(repo_path: &Path, branch: &str) -> Option<String> {
  let output = Command::new(consts::GIT_EXECUTABLE)
    .args(["config", &format!("branch.{branch}.remote")])
    .current_dir(repo_path)
    .output()
    .ok()?;

  if output.status.success() {
    let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !remote.is_empty() { Some(remote) } else { None }
  } else {
    None
  }
}
