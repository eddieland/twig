//! # Utility Functions
//!
//! Common utility functions and helpers for file operations, Git repository
//! validation, and shared functionality across the twig application.

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use twig_core::{detect_repository, detect_repository_from_path};

/// Resolve a repository path from a command line argument or current directory
pub fn resolve_repository_path(repo_arg: Option<&str>) -> Result<PathBuf> {
  match repo_arg {
    Some(path) => {
      let path_buf = PathBuf::from(path);
      if !path_buf.exists() {
        return Err(anyhow::anyhow!("Repository path does not exist: {}", path));
      }
      detect_repository_from_path(&path_buf).context(format!("Failed to detect repository at path: {path}"))
    }
    None => {
      // Try to detect the current repository
      detect_repository().context("No repository specified and not in a git repository")
    }
  }
}

/// Check if the current environment supports interactive input
///
/// Returns `false` if:
/// - stdin is not a TTY (e.g., piped input)
/// - Running in a CI environment (detected via CI environment variable)
/// - stdout is not a TTY
///
/// This is useful for determining whether to show interactive prompts
/// or fail immediately in automated environments.
pub fn is_interactive_environment() -> bool {
  // Check if stdin is a terminal
  if !std::io::stdin().is_terminal() {
    return false;
  }

  // Check if stdout is a terminal
  if !std::io::stdout().is_terminal() {
    return false;
  }

  // Check for common CI environment variables
  // https://docs.github.com/en/actions/learn-github-actions/variables#default-environment-variables
  if std::env::var("CI").is_ok()
    || std::env::var("GITHUB_ACTIONS").is_ok()
    || std::env::var("GITLAB_CI").is_ok()
    || std::env::var("CIRCLECI").is_ok()
    || std::env::var("TRAVIS").is_ok()
    || std::env::var("JENKINS_URL").is_ok()
    || std::env::var("BUILDKITE").is_ok()
  {
    return false;
  }

  true
}

/// Parse a list of commit hashes from a string or file path
///
/// Input can be:
/// - Comma-separated list of commit hashes: "abc123,def456,ghi789"
/// - Path to a file containing commit hashes (one per line)
///
/// Returns a vector of commit hash strings, or an error if parsing fails.
pub fn parse_skip_commits(input: &str) -> Result<Vec<String>> {
  use std::fs;
  use std::path::Path;

  // Check if input looks like a file path (contains path separators or has common file extensions)
  let looks_like_file = input.contains('/') 
    || input.contains('\\')
    || input.ends_with(".txt")
    || input.ends_with(".list");

  if looks_like_file {
    // Try to read as a file
    let path = Path::new(input);
    if path.exists() {
      let contents = fs::read_to_string(path)
        .context(format!("Failed to read skip-commits file: {}", input))?;
      
      let commits: Vec<String> = contents
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#')) // Skip empty lines and comments
        .map(|line| line.to_string())
        .collect();

      if commits.is_empty() {
        return Err(anyhow::anyhow!("Skip-commits file is empty or contains no valid commits: {}", input));
      }

      return Ok(commits);
    } else if input.contains(',') {
      // File doesn't exist, but input contains commas, so treat as comma-separated list
      // Fall through to comma-separated parsing
    } else {
      return Err(anyhow::anyhow!("Skip-commits file not found: {}", input));
    }
  }

  // Parse as comma-separated list
  let commits: Vec<String> = input
    .split(',')
    .map(|s| s.trim())
    .filter(|s| !s.is_empty())
    .map(|s| s.to_string())
    .collect();

  if commits.is_empty() {
    return Err(anyhow::anyhow!("No commit hashes provided in skip-commits"));
  }

  Ok(commits)
}

/// Validate that commit hashes look reasonable (basic format check)
///
/// Git commit hashes are 40-character hex strings (SHA-1) or 64-character hex strings (SHA-256),
/// but abbreviated hashes (7+ characters) are also commonly used.
pub fn validate_commit_hash(hash: &str) -> bool {
  // Allow abbreviated hashes (minimum 7 characters) and full hashes (40 or 64 characters)
  let len = hash.len();
  if len < 7 || len > 64 {
    return false;
  }

  // Check that all characters are valid hexadecimal
  hash.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;
  use twig_test_utils::GitRepoTestGuard;

  use super::*;

  // Test resolve_repository_path with a valid path
  #[test]
  fn test_resolve_repository_path_with_valid_path() {
    // Create a temporary directory to use as our "repository"
    let temp_dir = TempDir::new().unwrap();

    // This is a bit of a hack, but we can't easily mock these functions
    // without changing the code structure, so we'll just test the error path
    let result = resolve_repository_path(Some(temp_dir.path().to_str().unwrap()));

    // If the path exists but isn't a git repo, we'll get an error about failing to
    // detect repository
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Failed to detect repository"));
  }

  // Test resolve_repository_path with an invalid path
  #[test]
  fn test_resolve_repository_path_with_invalid_path() {
    let result = resolve_repository_path(Some("/path/that/does/not/exist"));
    assert!(result.is_err());
    assert!(
      result
        .unwrap_err()
        .to_string()
        .contains("Repository path does not exist")
    );
  }

  // Test resolve_repository_path with None (current directory)
  #[test]
  fn test_resolve_repository_path_with_none() {
    // Create a temporary git repository and change to its directory
    let git_repo = GitRepoTestGuard::new_and_change_dir();
    let git_repo_path = std::fs::canonicalize(git_repo.path()).unwrap();

    // Now test the function with None
    let result = resolve_repository_path(None);

    // The result should be Ok and contain our temporary directory path
    assert!(result.is_ok());
    let repo_path = std::fs::canonicalize(result.unwrap()).unwrap();
    assert_eq!(repo_path, git_repo_path);
  }

  #[test]
  fn test_parse_skip_commits_comma_separated() {
    let input = "abc123,def456,ghi789";
    let result = parse_skip_commits(input).unwrap();
    assert_eq!(result, vec!["abc123", "def456", "ghi789"]);
  }

  #[test]
  fn test_parse_skip_commits_with_spaces() {
    let input = "abc123, def456 , ghi789";
    let result = parse_skip_commits(input).unwrap();
    assert_eq!(result, vec!["abc123", "def456", "ghi789"]);
  }

  #[test]
  fn test_parse_skip_commits_from_file() {
    use std::io::Write;
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("commits.txt");
    
    let mut file = std::fs::File::create(&file_path).unwrap();
    writeln!(file, "abc123").unwrap();
    writeln!(file, "def456").unwrap();
    writeln!(file, "# This is a comment").unwrap();
    writeln!(file, "").unwrap(); // Empty line
    writeln!(file, "ghi789").unwrap();
    drop(file);

    let result = parse_skip_commits(file_path.to_str().unwrap()).unwrap();
    assert_eq!(result, vec!["abc123", "def456", "ghi789"]);
  }

  #[test]
  fn test_parse_skip_commits_empty_input() {
    let result = parse_skip_commits("");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No commit hashes provided"));
  }

  #[test]
  fn test_parse_skip_commits_file_not_found() {
    let result = parse_skip_commits("/path/that/does/not/exist.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
  }

  #[test]
  fn test_validate_commit_hash_valid() {
    assert!(validate_commit_hash("abc123d")); // 7 chars
    assert!(validate_commit_hash("abc123def456")); // 12 chars
    assert!(validate_commit_hash("abc123def456789012345678901234567890abcd")); // 40 chars (SHA-1)
    assert!(validate_commit_hash("abc123def456789012345678901234567890abcdef1234567890123456789012")); // 64 chars (SHA-256)
  }

  #[test]
  fn test_validate_commit_hash_invalid() {
    assert!(!validate_commit_hash("abc")); // Too short
    assert!(!validate_commit_hash("abc123")); // Too short (< 7)
    assert!(!validate_commit_hash("abc123def456789012345678901234567890abcdef12345678901234567890123")); // Too long (> 64)
    assert!(!validate_commit_hash("abc123g")); // Invalid character 'g'
    assert!(!validate_commit_hash("abc-123")); // Invalid character '-'
    assert!(!validate_commit_hash("abc 123")); // Invalid character ' '
  }
}

