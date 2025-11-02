//! Tests for enhanced CLI error handling and user experience
//!
//! This module tests Components 2.1 and 2.2:
//! - Enhanced Error Handling
//! - Improved User Experience

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use twig_cli::enhanced_errors::{ErrorCategory, ErrorHandler, TwigError};
use twig_cli::user_experience::{ColorOutput, ProgressIndicator, UserHints};

#[test]
fn test_enhanced_error_handling_repository_errors() {
  // Test Component 2.1: Enhanced Error Handling for repository errors
  let error = anyhow::anyhow!("not a git repository");
  let enhanced = ErrorHandler::handle_repository_error(error);

  assert_eq!(enhanced.category, ErrorCategory::GitRepository);
  assert!(enhanced.message.contains("Not in a git repository"));
  assert!(!enhanced.suggestions.is_empty());
  assert!(enhanced.suggestions.iter().any(|s| s.contains("git init")));
}

#[test]
fn test_enhanced_error_handling_branch_errors() {
  // Test branch not found error
  let error = anyhow::anyhow!("branch 'feature' not found");
  let enhanced = ErrorHandler::handle_branch_error("checkout", "feature", error);

  assert_eq!(enhanced.category, ErrorCategory::BranchOperation);
  assert!(enhanced.message.contains("Branch 'feature' not found"));
  assert!(!enhanced.suggestions.is_empty());

  // Test circular dependency error
  let error = anyhow::anyhow!("circular dependency detected");
  let enhanced = ErrorHandler::handle_branch_error("depend", "feature", error);

  assert!(enhanced.message.contains("Circular dependency"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("tree")));
}

#[test]
fn test_enhanced_error_handling_network_errors() {
  // Test timeout error
  let error = anyhow::anyhow!("request timed out");
  let enhanced = ErrorHandler::handle_network_error("GitHub", error);

  assert_eq!(enhanced.category, ErrorCategory::Network);
  assert!(enhanced.message.contains("timed out"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("connection")));

  // Test authentication error
  let error = anyhow::anyhow!("401 Unauthorized");
  let enhanced = ErrorHandler::handle_network_error("GitHub", error);

  assert!(enhanced.message.contains("authentication failed"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("netrc")));

  // Test rate limit error
  let error = anyhow::anyhow!("429 Too Many Requests");
  let enhanced = ErrorHandler::handle_network_error("GitHub", error);

  assert!(enhanced.message.contains("rate limit"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("wait")));
}

#[test]
fn test_enhanced_error_handling_file_errors() {
  // Test permission denied
  let error = anyhow::anyhow!("Permission denied");
  let enhanced = ErrorHandler::handle_file_error("write", "/path/to/file", error);

  assert_eq!(enhanced.category, ErrorCategory::FileSystem);
  assert!(enhanced.message.contains("Permission denied"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("permissions")));

  // Test file not found
  let error = anyhow::anyhow!("No such file or directory");
  let enhanced = ErrorHandler::handle_file_error("read", "/missing/file", error);

  assert!(enhanced.message.contains("File not found"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("path")));
}

#[test]
fn test_enhanced_error_handling_git_command_errors() {
  // Test git not found
  let error = anyhow::anyhow!("git: command not found");
  let enhanced = ErrorHandler::handle_git_command_error("git status", error);

  assert_eq!(enhanced.category, ErrorCategory::ExternalCommand);
  assert!(enhanced.message.contains("Git command not found"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("Install Git")));

  // Test merge conflict
  let error = anyhow::anyhow!("merge conflict in file.txt");
  let enhanced = ErrorHandler::handle_git_command_error("git rebase", error);

  assert!(enhanced.message.contains("merge conflict"));
  assert!(enhanced.suggestions.iter().any(|s| s.contains("git status")));
}

#[test]
fn test_twig_error_creation_and_chaining() {
  let error = TwigError::new(ErrorCategory::BranchOperation, "Test error")
    .with_details("Additional context")
    .with_suggestion("First suggestion")
    .with_suggestions(["Second suggestion", "Third suggestion"])
    .with_exit_code(2);

  assert_eq!(error.category, ErrorCategory::BranchOperation);
  assert_eq!(error.message, "Test error");
  assert_eq!(error.details, Some("Additional context".to_string()));
  assert_eq!(error.suggestions.len(), 3);
  assert_eq!(error.exit_code, 2);

  // Test display doesn't panic
  let display_str = format!("{}", error);
  assert_eq!(display_str, "Test error");
}

#[test]
fn test_progress_indicator_creation() {
  // Test Component 2.2: Progress indicators
  let mut progress = ProgressIndicator::new("Testing operation");

  // Test that it can be created and doesn't panic
  progress.start();

  // Small delay to let spinner run
  std::thread::sleep(Duration::from_millis(50));

  progress.finish(Some("Operation completed"));

  // Should not crash
}

#[test]
fn test_color_output_functionality() {
  // Test Component 2.2: Colored output
  let output = ColorOutput::new();

  // Test that color output can be created and methods don't panic
  let result = output.format_branch_name("feature/test", true);
  assert!(result.contains("feature/test"));

  let result = output.format_branch_name("main", false);
  assert!(result.contains("main"));

  let result = output.format_git_status("up-to-date");
  assert_eq!(result, "up-to-date");
}

#[test]
fn test_user_hints_branch_suggestions() {
  // Test Component 2.2: Helpful hints for user mistakes
  let branches = vec![
    "feature/login".to_string(),
    "feature/signup".to_string(),
    "main".to_string(),
    "develop".to_string(),
  ];

  // Test exact match (should return None)
  assert_eq!(UserHints::suggest_branch_name("main", &branches), None);

  // Test close match (should suggest correction)
  let suggestion = UserHints::suggest_branch_name("featur/login", &branches);
  assert_eq!(suggestion, Some("feature/login".to_string()));

  // Test typo in existing branch
  let suggestion = UserHints::suggest_branch_name("mian", &branches);
  assert_eq!(suggestion, Some("main".to_string()));

  // Test completely different (should return None)
  let suggestion = UserHints::suggest_branch_name("completely-different-branch", &branches);
  assert_eq!(suggestion, None);
}

#[test]
fn test_user_hints_git_workflow_suggestions() {
  // Test git workflow suggestions based on error context
  let suggestions = UserHints::suggest_git_workflow("not a git repository");
  assert!(suggestions.contains(&"git init".to_string()));
  assert!(suggestions.contains(&"git clone <repository-url>".to_string()));

  let suggestions = UserHints::suggest_git_workflow("branch 'feature' not found");
  assert!(suggestions.contains(&"git branch -a".to_string()));
  assert!(suggestions.contains(&"git checkout -b <branch-name>".to_string()));

  let suggestions = UserHints::suggest_git_workflow("merge conflict detected");
  assert!(suggestions.contains(&"git status".to_string()));
  assert!(suggestions.contains(&"git rebase --continue".to_string()));

  let suggestions = UserHints::suggest_git_workflow("uncommitted changes in working directory");
  assert!(suggestions.contains(&"git add .".to_string()));
  assert!(suggestions.contains(&"git stash".to_string()));
}

#[test]
fn test_user_hints_command_help() {
  // Test contextual command help
  let help = UserHints::command_help_hint("branch", Some("depend"));
  assert!(help.is_some());
  assert!(help.unwrap().contains("parent-child"));

  let help = UserHints::command_help_hint("cascade", None);
  assert!(help.is_some());
  assert!(help.unwrap().contains("cascading rebase"));

  let help = UserHints::command_help_hint("unknown", None);
  assert!(help.is_none());
}

#[test]
fn test_levenshtein_distance_calculations() {
  // Test the Levenshtein distance function used for suggestions
  use twig_cli::user_experience::*;

  // Access the private function through the public interface by testing
  // suggestions
  let branches = vec!["feature".to_string(), "main".to_string()];

  // Distance of 1 (substitution)
  assert_eq!(
    UserHints::suggest_branch_name("featur", &branches),
    Some("feature".to_string())
  );

  // Distance of 1 (insertion)
  assert_eq!(
    UserHints::suggest_branch_name("featuree", &branches),
    Some("feature".to_string())
  );

  // Distance of 2 should still suggest
  assert_eq!(
    UserHints::suggest_branch_name("featue", &branches),
    Some("feature".to_string())
  );

  // Distance too large should not suggest
  assert_eq!(UserHints::suggest_branch_name("completely_different", &branches), None);
}

#[test]
fn test_error_category_matching() {
  // Test that error categories are properly assigned
  let repo_error = ErrorHandler::handle_repository_error(anyhow::anyhow!("not in git repo"));
  assert_eq!(repo_error.category, ErrorCategory::GitRepository);

  let branch_error = ErrorHandler::handle_branch_error("create", "test", anyhow::anyhow!("branch exists"));
  assert_eq!(branch_error.category, ErrorCategory::BranchOperation);

  let network_error = ErrorHandler::handle_network_error("API", anyhow::anyhow!("timeout"));
  assert_eq!(network_error.category, ErrorCategory::Network);

  let file_error = ErrorHandler::handle_file_error("read", "path", anyhow::anyhow!("permission denied"));
  assert_eq!(file_error.category, ErrorCategory::FileSystem);
}

#[test]
fn test_enhanced_error_display_formatting() {
  // Test that enhanced error display includes all components
  let error = TwigError::new(ErrorCategory::BranchOperation, "Test operation failed")
    .with_details("Detailed explanation of what went wrong")
    .with_suggestions(["Try this first", "Or try this second"]);

  // Test display doesn't panic and includes basic message
  let display = format!("{}", error);
  assert!(display.contains("Test operation failed"));

  // Test that details and suggestions are available
  assert!(error.details.is_some());
  assert_eq!(error.suggestions.len(), 2);
}

// Integration test for error handling in repository operations
#[test]
fn test_error_handling_integration() -> Result<()> {
  use twig_test_utils::git::GitRepoTestGuard;

  // Create a test repository
  let _git_repo = GitRepoTestGuard::new();

  // Test that our error handling works with real operations
  let error = anyhow::anyhow!("Simulated repository error");
  let enhanced = ErrorHandler::handle_repository_error(error);

  // Verify error has proper structure for integration
  assert!(!enhanced.message.is_empty());
  assert!(!enhanced.suggestions.is_empty());
  assert!(enhanced.exit_code > 0);

  Ok(())
}
