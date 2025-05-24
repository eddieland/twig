use colored::Colorize;

/// Print a success message
pub fn print_success(message: &str) {
  println!("{} {}", "✓".green().bold(), message);
}

/// Print an error message
pub fn print_error(message: &str) {
  eprintln!("{} {}", "✗".red().bold(), message);
}

/// Print a warning message
pub fn print_warning(message: &str) {
  println!("{} {}", "!".yellow().bold(), message);
}

/// Print an info message
pub fn print_info(message: &str) {
  println!("{} {}", "ℹ".blue().bold(), message);
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
