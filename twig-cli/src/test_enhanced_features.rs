//! Tests for enhanced error handling functionality only
//!
//! This module tests our enhanced error handling in isolation

#[cfg(test)]
mod tests {
  use anyhow::Result;

  // Test that our enhanced error handling types are properly defined
  #[test]
  fn test_error_categories_exist() {
    // Just ensure the types compile and can be instantiated
    use crate::enhanced_errors::{ErrorCategory, TwigError};

    let categories = vec![
      ErrorCategory::GitRepository,
      ErrorCategory::BranchOperation,
      ErrorCategory::FileSystem,
      ErrorCategory::Network,
      ErrorCategory::Configuration,
      ErrorCategory::UserInput,
      ErrorCategory::ExternalCommand,
    ];

    assert_eq!(categories.len(), 7);

    // Test basic error creation
    let error = TwigError::new(ErrorCategory::BranchOperation, "Test error");
    assert_eq!(error.message, "Test error");
    assert_eq!(error.category, ErrorCategory::BranchOperation);
  }

  #[test]
  fn test_error_handler_methods_compile() {
    use crate::enhanced_errors::ErrorHandler;

    // Test that all handler methods exist and compile
    let repo_error = ErrorHandler::handle_repository_error(anyhow::anyhow!("test"));
    assert!(!repo_error.message.is_empty());

    let branch_error = ErrorHandler::handle_branch_error("test", "branch", anyhow::anyhow!("test"));
    assert!(!branch_error.message.is_empty());

    let file_error = ErrorHandler::handle_file_error("test", "path", anyhow::anyhow!("test"));
    assert!(!file_error.message.is_empty());

    let network_error = ErrorHandler::handle_network_error("service", anyhow::anyhow!("test"));
    assert!(!network_error.message.is_empty());

    let config_error = ErrorHandler::handle_config_error("config", anyhow::anyhow!("test"));
    assert!(!config_error.message.is_empty());

    let git_error = ErrorHandler::handle_git_command_error("git", anyhow::anyhow!("test"));
    assert!(!git_error.message.is_empty());
  }

  #[test]
  fn test_user_experience_types_compile() {
    use crate::user_experience::{ColorOutput, ProgressIndicator, UserHints};

    // Test progress indicator
    let mut progress = ProgressIndicator::new("Test");
    // Don't start it to avoid threading issues in tests

    // Test color output
    let output = ColorOutput::new();
    let formatted = output.format_branch_name("test", true);
    assert!(formatted.contains("test"));

    // Test user hints
    let branches = vec!["main".to_string(), "feature".to_string()];
    let suggestion = UserHints::suggest_branch_name("mai", &branches);
    assert_eq!(suggestion, Some("main".to_string()));

    let git_suggestions = UserHints::suggest_git_workflow("not a git repository");
    assert!(!git_suggestions.is_empty());
  }
}
