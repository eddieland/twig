use colored::Colorize;
use emojis;

// Helper function to safely get an emoji or fallback to a default character
fn get_emoji_or_default(name: &str, default: &str) -> String {
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
          "success" => "Success".green().to_string(),
          "failure" => "Failure".red().to_string(),
          "neutral" => "Neutral".yellow().to_string(),
          "cancelled" => "Cancelled".yellow().to_string(),
          "timed_out" => "Timed Out".red().to_string(),
          "action_required" => "Action Required".yellow().to_string(),
          _ => conclusion.to_string(),
        }
      } else {
        "Completed".to_string()
      }
    }
    "in_progress" => "In Progress".yellow().to_string(),
    "queued" => "Queued".cyan().to_string(),
    _ => status.to_string(),
  }
}
