//! # Output Formatting
//!
//! Provides formatted output functions with colors, emojis, and consistent
//! styling for user-facing messages and terminal output.

use owo_colors::OwoColorize;
use {clap, emojis};

/// Enum representing different color modes for output
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
  /// Enable colored output
  Yes,
  /// Enable colored output (alias for Yes)
  Always,
  /// Automatically detect if colors should be used based on terminal
  /// capabilities
  Auto,
  /// Disable colored output
  No,
  /// Disable colored output (alias for No)
  Never,
}

/// Helper function to safely get an emoji or fallback to a default character
pub fn get_emoji_or_default(name: &str, default: &str) -> String {
  match emojis::get_by_shortcode(name) {
    Some(emoji) => emoji.to_string(),
    None => default.to_string(),
  }
}

/// Print a success message
pub fn print_success(message: &str) {
  let check = get_emoji_or_default("check_mark", "✓");
  println!("{} {}", check.green().bold(), message);
}

/// Print an error message
pub fn print_error(message: &str) {
  let cross = get_emoji_or_default("cross_mark", "✗");
  eprintln!("{} {}", cross.red().bold(), message);
}

/// Print a warning message
pub fn print_warning(message: &str) {
  let warning = get_emoji_or_default("warning", "⚠");
  println!("{} {}", warning.yellow().bold(), message);
}

/// Print an info message
pub fn print_info(message: &str) {
  let info = get_emoji_or_default("information", "ℹ");
  println!("{} {}", info.blue().bold(), message);
}

/// Print a section header
pub fn print_header(header: &str) {
  println!("\n{}", header.blue().bold());
}

/// Format a repository path
pub fn format_repo_path(path: &str) -> String {
  path.bright_green().to_string()
}

/// Format a repository name
pub fn format_repo_name(name: &str) -> String {
  name.bright_cyan().bold().to_string()
}

/// Format a timestamp
pub fn format_timestamp(timestamp: &str) -> String {
  timestamp.yellow().to_string()
}

/// Format a command or command example
pub fn format_command(cmd: &str) -> String {
  cmd.purple().to_string()
}

/// Format a GitHub PR review status
pub fn format_pr_review_status(state: &str) -> String {
  match state {
    "APPROVED" => state.green().to_string(),
    "CHANGES_REQUESTED" => state.red().to_string(),
    "COMMENTED" => state.yellow().to_string(),
    _ => state.to_string(),
  }
}

/// Format a GitHub check run status
pub fn format_check_status(status: &str, conclusion: Option<&str>) -> String {
  match status {
    "completed" => {
      if let Some(conclusion) = conclusion {
        match conclusion {
          "success" => conclusion.green().to_string(),
          "failure" => conclusion.red().to_string(),
          "cancelled" => conclusion.yellow().to_string(),
          "skipped" => conclusion.bright_black().to_string(),
          _ => conclusion.to_string(),
        }
      } else {
        status.to_string()
      }
    }
    "in_progress" => status.yellow().to_string(),
    "queued" => status.blue().to_string(),
    _ => status.to_string(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_emoji_or_default() {
    // Test with a known emoji
    let result = get_emoji_or_default("check_mark", "✓");
    assert!(!result.is_empty());

    // Test with unknown emoji
    let result = get_emoji_or_default("nonexistent_emoji", "fallback");
    assert_eq!(result, "fallback");
  }

  #[test]
  fn test_format_functions() {
    let path = format_repo_path("/test/path");
    assert!(!path.is_empty());

    let name = format_repo_name("test-repo");
    assert!(!name.is_empty());

    let timestamp = format_timestamp("2023-01-01");
    assert!(!timestamp.is_empty());

    let command = format_command("git status");
    assert!(!command.is_empty());
  }

  #[test]
  fn test_pr_review_status_formatting() {
    assert!(!format_pr_review_status("APPROVED").is_empty());
    assert!(!format_pr_review_status("CHANGES_REQUESTED").is_empty());
    assert!(!format_pr_review_status("COMMENTED").is_empty());
    assert!(!format_pr_review_status("UNKNOWN").is_empty());
  }

  #[test]
  fn test_check_status_formatting() {
    assert!(!format_check_status("completed", Some("success")).is_empty());
    assert!(!format_check_status("completed", Some("failure")).is_empty());
    assert!(!format_check_status("in_progress", None).is_empty());
    assert!(!format_check_status("queued", None).is_empty());
  }
}
