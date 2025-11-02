//! Enhanced error handling for twig CLI commands
//!
//! This module implements Component 2.1: Enhanced Error Handling
//! - Standardizes error message format across all commands
//! - Provides actionable error messages with suggested fixes
//! - Implements proper error context propagation
//! - Adds debug logging for troubleshooting
//! - Handles git command failures gracefully

use std::fmt;

use anyhow::Result;
use tracing::{debug, error, warn};
use twig_core::output::{print_error, print_info};

/// Standardized error categories for consistent handling
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
  /// Git repository related errors
  GitRepository,
  /// Branch operation errors
  BranchOperation,
  /// File system access errors
  FileSystem,
  /// Network/API related errors
  Network,
  /// Configuration errors
  Configuration,
  /// User input validation errors
  UserInput,
  /// External command execution errors
  ExternalCommand,
}

/// Enhanced error information with context and suggestions
#[derive(Debug, Clone)]
pub struct TwigError {
  /// The error category
  pub category: ErrorCategory,
  /// Primary error message
  pub message: String,
  /// Detailed description for debug purposes
  pub details: Option<String>,
  /// Suggested actions to resolve the error
  pub suggestions: Vec<String>,
  /// Exit code to use when this error causes program termination
  pub exit_code: i32,
}

impl fmt::Display for TwigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

impl std::error::Error for TwigError {}

impl TwigError {
  /// Create a new TwigError
  pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
    Self {
      category,
      message: message.into(),
      details: None,
      suggestions: Vec::new(),
      exit_code: 1,
    }
  }

  /// Add detailed context information
  pub fn with_details(mut self, details: impl Into<String>) -> Self {
    self.details = Some(details.into());
    self
  }

  /// Add a suggested action to resolve the error
  pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
    self.suggestions.push(suggestion.into());
    self
  }

  /// Add multiple suggestions
  pub fn with_suggestions<I, S>(mut self, suggestions: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    self.suggestions.extend(suggestions.into_iter().map(|s| s.into()));
    self
  }

  /// Set custom exit code
  pub fn with_exit_code(mut self, code: i32) -> Self {
    self.exit_code = code;
    self
  }

  /// Display this error with enhanced formatting and suggestions
  pub fn display_enhanced(&self) {
    error!(category = ?self.category, message = %self.message, "TwigError occurred");

    print_error(&format!("Error: {}", self.message));

    if let Some(details) = &self.details {
      debug!("Error details: {}", details);
      print_info(&format!("Details: {}", details));
    }

    if !self.suggestions.is_empty() {
      print_info("Suggested solutions:");
      for (i, suggestion) in self.suggestions.iter().enumerate() {
        print_info(&format!("  {}. {}", i + 1, suggestion));
      }
    }
  }
}

/// Enhanced error handling utilities
pub struct ErrorHandler;

impl ErrorHandler {
  /// Handle repository detection errors with enhanced suggestions
  pub fn handle_repository_error(error: anyhow::Error) -> TwigError {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("not a git repository") || error_str.contains("not in a git repository") {
      TwigError::new(ErrorCategory::GitRepository, "Not in a git repository")
        .with_details("The current directory is not inside a git repository")
        .with_suggestions([
          "Navigate to a git repository directory",
          "Initialize a new git repository with 'git init'",
          "Clone an existing repository with 'git clone <url>'",
          "Use the --repo flag to specify a repository path",
        ])
    } else if error_str.contains("permission denied") {
      TwigError::new(ErrorCategory::FileSystem, "Permission denied accessing git repository")
        .with_details(&format!("Git operation failed: {}", error))
        .with_suggestions([
          "Check file permissions on the repository directory",
          "Ensure you have read/write access to the repository",
          "Try running with appropriate user permissions",
        ])
    } else {
      TwigError::new(ErrorCategory::GitRepository, "Git repository access failed")
        .with_details(&format!("Underlying error: {}", error))
        .with_suggestion("Verify the repository is in a valid state")
    }
  }

  /// Handle branch operation errors with context-aware suggestions
  pub fn handle_branch_error(operation: &str, branch_name: &str, error: anyhow::Error) -> TwigError {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("branch") && error_str.contains("not found") {
      TwigError::new(
        ErrorCategory::BranchOperation,
        format!("Branch '{}' not found", branch_name),
      )
      .with_details(&format!(
        "Failed to {} branch '{}': branch does not exist",
        operation, branch_name
      ))
      .with_suggestions([
        format!("Create the branch with 'git checkout -b {}'", branch_name),
        "List available branches with 'git branch -a'".to_string(),
        "Check for typos in the branch name".to_string(),
      ])
    } else if error_str.contains("already exists") {
      TwigError::new(
        ErrorCategory::BranchOperation,
        format!("Branch '{}' already exists", branch_name),
      )
      .with_details(&format!(
        "Cannot {} branch '{}': branch already exists",
        operation, branch_name
      ))
      .with_suggestions([
        "Use a different branch name".to_string(),
        format!("Delete the existing branch with 'git branch -d {}'", branch_name),
        "Switch to the existing branch if that's what you intended".to_string(),
      ])
    } else if error_str.contains("circular dependency") {
      TwigError::new(ErrorCategory::BranchOperation, "Circular dependency detected")
        .with_details(&format!(
          "Cannot create dependency for '{}': would create a circular dependency",
          branch_name
        ))
        .with_suggestions([
          "Review your branch dependency structure",
          "Use 'twig tree' to visualize current dependencies",
          "Remove conflicting dependencies before adding new ones",
        ])
    } else {
      TwigError::new(
        ErrorCategory::BranchOperation,
        format!("Branch operation '{}' failed", operation),
      )
      .with_details(&format!("Failed to {} branch '{}': {}", operation, branch_name, error))
      .with_suggestions([
        "Verify the repository is in a clean state",
        "Check that you have the necessary permissions",
        "Try the operation again after resolving any conflicts",
      ])
    }
  }

  /// Handle file system errors with helpful context
  pub fn handle_file_error(operation: &str, path: &str, error: anyhow::Error) -> TwigError {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("permission denied") {
      TwigError::new(
        ErrorCategory::FileSystem,
        format!("Permission denied: cannot {} {}", operation, path),
      )
      .with_details(&format!("File system error: {}", error))
      .with_suggestions([
        "Check file permissions on the target path",
        "Ensure you have appropriate read/write access",
        "Verify the directory exists and is writable",
      ])
    } else if error_str.contains("no such file or directory") {
      TwigError::new(ErrorCategory::FileSystem, format!("File not found: {}", path))
        .with_details(&format!("Cannot {}: file or directory does not exist", operation))
        .with_suggestions([
          "Verify the path is correct",
          "Check for typos in the file path",
          "Create the directory if it should exist",
        ])
    } else {
      TwigError::new(
        ErrorCategory::FileSystem,
        format!("File system error during {}", operation),
      )
      .with_details(&format!("Error accessing '{}': {}", path, error))
      .with_suggestion("Check file system permissions and path validity")
    }
  }

  /// Handle network/API errors with retry suggestions
  pub fn handle_network_error(service: &str, error: anyhow::Error) -> TwigError {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("timeout") || error_str.contains("timed out") {
      TwigError::new(ErrorCategory::Network, format!("{} request timed out", service))
        .with_details(&format!("Network timeout: {}", error))
        .with_suggestions([
          "Check your internet connection",
          "Try the operation again in a few moments",
          "Verify the service is accessible from your network",
        ])
    } else if error_str.contains("unauthorized") || error_str.contains("401") {
      TwigError::new(ErrorCategory::Network, format!("{} authentication failed", service))
        .with_details(&format!("Authentication error: {}", error))
        .with_suggestions([
          "Check your credentials in ~/.netrc",
          "Verify your authentication token is valid",
          "Re-authenticate with the service if needed",
        ])
    } else if error_str.contains("rate limit") || error_str.contains("429") {
      TwigError::new(ErrorCategory::Network, format!("{} rate limit exceeded", service))
        .with_details(&format!("API rate limiting: {}", error))
        .with_suggestions([
          "Wait a few minutes before retrying",
          "Consider using authentication to increase rate limits",
          "Reduce the frequency of API requests",
        ])
    } else {
      TwigError::new(ErrorCategory::Network, format!("{} network error", service))
        .with_details(&format!("Network error: {}", error))
        .with_suggestions([
          "Check your internet connection",
          "Verify the service URL is correct",
          "Try again later if the service is experiencing issues",
        ])
    }
  }

  /// Handle configuration errors with setup guidance
  pub fn handle_config_error(config_type: &str, error: anyhow::Error) -> TwigError {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("parse") || error_str.contains("invalid") {
      TwigError::new(
        ErrorCategory::Configuration,
        format!("Invalid {} configuration", config_type),
      )
      .with_details(&format!("Configuration parse error: {}", error))
      .with_suggestions([
        "Check the configuration file syntax",
        "Verify all required fields are present",
        "Compare with example configuration files",
        "Reset to default configuration if necessary",
      ])
    } else if error_str.contains("not found") || error_str.contains("no such file") {
      TwigError::new(
        ErrorCategory::Configuration,
        format!("{} configuration not found", config_type),
      )
      .with_details(&format!("Configuration file missing: {}", error))
      .with_suggestions([
        "Run 'twig init' to create default configuration",
        "Check the configuration file path",
        "Create a minimal configuration file manually",
      ])
    } else {
      TwigError::new(
        ErrorCategory::Configuration,
        format!("{} configuration error", config_type),
      )
      .with_details(&format!("Configuration error: {}", error))
      .with_suggestion("Review and fix the configuration settings")
    }
  }

  /// Handle git command execution errors
  pub fn handle_git_command_error(command: &str, error: anyhow::Error) -> TwigError {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("not found") && error_str.contains("git") {
      TwigError::new(ErrorCategory::ExternalCommand, "Git command not found")
        .with_details("The 'git' command is not available in your PATH")
        .with_suggestions([
          "Install Git from https://git-scm.com/",
          "Ensure Git is installed and available in your PATH",
          "Restart your terminal after installing Git",
        ])
    } else if error_str.contains("merge conflict") {
      TwigError::new(ErrorCategory::GitRepository, "Git merge conflict detected")
        .with_details(&format!("Git command '{}' failed due to merge conflicts", command))
        .with_suggestions([
          "Resolve merge conflicts manually",
          "Use 'git status' to see conflicted files",
          "Run 'git add .' after resolving conflicts",
          "Continue with 'git rebase --continue' or 'git merge --continue'",
        ])
    } else {
      TwigError::new(
        ErrorCategory::ExternalCommand,
        format!("Git command failed: {}", command),
      )
      .with_details(&format!("Git command error: {}", error))
      .with_suggestions([
        "Check that the repository is in a clean state",
        "Verify you have necessary permissions",
        "Review git status and resolve any issues",
      ])
    }
  }

  /// Convert a generic anyhow::Error to a TwigError with enhanced context
  pub fn from_anyhow(error: anyhow::Error, context: &str) -> TwigError {
    warn!("Converting anyhow error to TwigError: {} - {}", context, error);

    TwigError::new(
      ErrorCategory::Configuration, // Default category
      format!("{}: {}", context, error),
    )
    .with_details(&format!("Original error: {}", error))
    .with_suggestion("Check the operation parameters and try again")
  }
}

/// Extension trait for Result types to add enhanced error handling
pub trait ResultExt<T> {
  /// Convert to TwigError with enhanced context
  fn with_twig_context(self, category: ErrorCategory, message: &str) -> Result<T, TwigError>;

  /// Add suggestions to any error result
  fn with_suggestions<I, S>(self, suggestions: I) -> Result<T, TwigError>
  where
    I: IntoIterator<Item = S>,
    S: Into<String>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
  E: std::error::Error + Send + Sync + 'static,
{
  fn with_twig_context(self, category: ErrorCategory, message: &str) -> Result<T, TwigError> {
    self.map_err(|e| TwigError::new(category, message).with_details(&format!("Underlying error: {}", e)))
  }

  fn with_suggestions<I, S>(self, suggestions: I) -> Result<T, TwigError>
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    self.map_err(|e| TwigError::new(ErrorCategory::Configuration, format!("{}", e)).with_suggestions(suggestions))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_twig_error_creation() {
    let error = TwigError::new(ErrorCategory::BranchOperation, "Test error")
      .with_details("Test details")
      .with_suggestion("Test suggestion")
      .with_exit_code(2);

    assert_eq!(error.category, ErrorCategory::BranchOperation);
    assert_eq!(error.message, "Test error");
    assert_eq!(error.details, Some("Test details".to_string()));
    assert_eq!(error.suggestions, vec!["Test suggestion"]);
    assert_eq!(error.exit_code, 2);
  }

  #[test]
  fn test_repository_error_handling() {
    let error = anyhow::anyhow!("not a git repository");
    let twig_error = ErrorHandler::handle_repository_error(error);

    assert_eq!(twig_error.category, ErrorCategory::GitRepository);
    assert!(twig_error.message.contains("Not in a git repository"));
    assert!(!twig_error.suggestions.is_empty());
  }

  #[test]
  fn test_branch_error_handling() {
    let error = anyhow::anyhow!("branch 'feature' not found");
    let twig_error = ErrorHandler::handle_branch_error("checkout", "feature", error);

    assert_eq!(twig_error.category, ErrorCategory::BranchOperation);
    assert!(twig_error.message.contains("Branch 'feature' not found"));
    assert!(!twig_error.suggestions.is_empty());
  }
}
