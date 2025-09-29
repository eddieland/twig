//! # Jira Ticket Parser
//!
//! Provides flexible parsing and normalization of Jira ticket identifiers.
//! Supports various input formats while maintaining a canonical output format.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration for Jira ticket parsing behavior and connection settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JiraParsingConfig {
  /// The parsing mode to use
  pub mode: JiraParsingMode,
  
  /// Jira host URL (e.g., https://company.atlassian.net)
  #[serde(default)]
  pub host: Option<String>,
}

impl Default for JiraParsingConfig {
  fn default() -> Self {
    Self {
      mode: JiraParsingMode::Flexible,
      host: None,
    }
  }
}

/// Parsing mode for Jira tickets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JiraParsingMode {
  /// Strict mode: Only accepts ME-1234 format (old behavior)
  Strict,
  /// Flexible mode: Accepts ME-1234, ME1234, me1234, Me1234, etc.
  Flexible,
}

/// Errors that can occur during Jira ticket parsing
#[derive(Debug, Error)]
pub enum JiraParseError {
  #[error("Invalid ticket format: '{0}' does not match any supported pattern")]
  InvalidFormat(String),
  #[error("Project code too short: '{0}' must be at least 2 characters")]
  ProjectTooShort(String),
  #[error("Missing ticket number in: '{0}'")]
  MissingNumber(String),
}

/// Jira ticket parser with configurable parsing modes
pub struct JiraTicketParser {
  config: JiraParsingConfig,
}

// Regex patterns for different parsing modes
static STRICT_PATTERN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[A-Z]{2,}-\d+$").expect("Failed to compile strict Jira regex"));

static FLEXIBLE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  vec![
    // ME-1234, me-1234, Me-1234, etc. (with hyphen)
    Regex::new(r"^([A-Za-z]{2,})-(\d+)$").expect("Failed to compile flexible Jira regex with hyphen"),
    // ME1234, me1234, Me1234, etc. (without hyphen)
    Regex::new(r"^([A-Za-z]{2,})(\d+)$").expect("Failed to compile flexible Jira regex without hyphen"),
  ]
});

// Pattern for extracting from commit messages (flexible version)
static COMMIT_MESSAGE_PATTERN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^([A-Za-z]{2,}[-]?\d+):").expect("Failed to compile commit message Jira regex"));

// Pattern for extracting from commit messages (strict version)
static COMMIT_MESSAGE_STRICT_PATTERN: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^([A-Z]{2,}-\d+):").expect("Failed to compile strict commit message Jira regex"));

impl JiraTicketParser {
  /// Create a new parser with the given configuration
  pub fn new(config: JiraParsingConfig) -> Self {
    Self { config }
  }

  /// Create a new parser with default configuration (flexible mode)
  pub fn new_default() -> Self {
    Self::new(JiraParsingConfig::default())
  }

  /// Create a new parser in flexible mode
  pub fn new_flexible() -> Self {
    Self::new(JiraParsingConfig {
      mode: JiraParsingMode::Flexible,
      host: None,
    })
  }

  /// Create a new parser in strict mode
  pub fn new_strict() -> Self {
    Self::new(JiraParsingConfig {
      mode: JiraParsingMode::Strict,
      host: None,
    })
  }

  /// Parse a Jira ticket identifier from user input
  pub fn parse(&self, input: &str) -> Result<String, JiraParseError> {
    let input = input.trim();

    if input.is_empty() {
      return Err(JiraParseError::InvalidFormat(input.to_string()));
    }

    match &self.config.mode {
      JiraParsingMode::Strict => self.parse_strict(input),
      JiraParsingMode::Flexible => self.parse_flexible(input),
    }
  }

  /// Parse using strict mode (current behavior)
  fn parse_strict(&self, input: &str) -> Result<String, JiraParseError> {
    if STRICT_PATTERN.is_match(input) {
      Ok(input.to_string())
    } else {
      Err(JiraParseError::InvalidFormat(input.to_string()))
    }
  }

  /// Parse using flexible mode (accepts various formats)
  fn parse_flexible(&self, input: &str) -> Result<String, JiraParseError> {
    for pattern in FLEXIBLE_PATTERNS.iter() {
      if let Some(captures) = pattern.captures(input) {
        let project = captures.get(1).unwrap().as_str();
        let number = captures.get(2).unwrap().as_str();

        if project.len() < 2 {
          return Err(JiraParseError::ProjectTooShort(project.to_string()));
        }

        return Ok(self.normalize(project, number));
      }
    }

    Err(JiraParseError::InvalidFormat(input.to_string()))
  }

  /// Extract Jira ticket from commit message
  pub fn extract_from_commit_message(&self, message: &str) -> Option<String> {
    match &self.config.mode {
      JiraParsingMode::Strict => COMMIT_MESSAGE_STRICT_PATTERN
        .captures(message)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string()),
      JiraParsingMode::Flexible => {
        COMMIT_MESSAGE_PATTERN
          .captures(message)
          .and_then(|caps| caps.get(1))
          .and_then(|m| {
            let ticket = m.as_str();
            // Try to parse and normalize the extracted ticket
            self.parse(ticket).ok()
          })
      }
    }
  }

  /// Check if input is a valid Jira ticket format
  pub fn is_valid(&self, input: &str) -> bool {
    self.parse(input).is_ok()
  }

  /// Normalize a ticket to canonical format (PROJECT-NUMBER)
  fn normalize(&self, project: &str, number: &str) -> String {
    format!("{}-{}", project.to_uppercase(), number)
  }

  /// Get the current configuration
  pub fn config(&self) -> &JiraParsingConfig {
    &self.config
  }

  /// Update the parser configuration
  pub fn set_config(&mut self, config: JiraParsingConfig) {
    self.config = config;
  }
}

/// Create a Jira parser with configuration loaded from the config directories.
/// Returns `None` if no configuration is found or if config loading fails.
///
/// This is a convenience function that encapsulates the common pattern of:
/// 1. Getting config directories
/// 2. Loading Jira configuration
/// 3. Creating a parser with that configuration
///
/// # Returns
/// - `Some(JiraTicketParser)` if configuration is successfully loaded
/// - `None` if no config directories are available or Jira config loading fails
pub fn create_jira_parser() -> Option<JiraTicketParser> {
  use crate::get_config_dirs;

  let config_dirs = get_config_dirs().ok()?;
  let jira_config = config_dirs.load_jira_config().ok()?;
  Some(JiraTicketParser::new(jira_config))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_strict_mode_valid_formats() {
    let parser = JiraTicketParser::new_strict();

    assert_eq!(parser.parse("ME-1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("PROJECT-999").unwrap(), "PROJECT-999");
    assert_eq!(parser.parse("AB-1").unwrap(), "AB-1");
  }

  #[test]
  fn test_strict_mode_invalid_formats() {
    let parser = JiraTicketParser::new_strict();

    assert!(parser.parse("me-1234").is_err());
    assert!(parser.parse("ME1234").is_err());
    assert!(parser.parse("M-123").is_err()); // Too short project
    assert!(parser.parse("ME-").is_err());
    assert!(parser.parse("-1234").is_err());
  }

  #[test]
  fn test_flexible_mode_case_variations() {
    let parser = JiraTicketParser::new_flexible();

    assert_eq!(parser.parse("ME-1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("me-1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("Me-1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("mE-1234").unwrap(), "ME-1234");
  }

  #[test]
  fn test_flexible_mode_hyphen_variations() {
    let parser = JiraTicketParser::new_flexible();

    assert_eq!(parser.parse("ME1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("me1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("Me1234").unwrap(), "ME-1234");
    assert_eq!(parser.parse("mE1234").unwrap(), "ME-1234");
  }

  #[test]
  fn test_flexible_mode_invalid_formats() {
    let parser = JiraTicketParser::new_flexible();

    assert!(parser.parse("M-123").is_err()); // Too short project
    assert!(parser.parse("ME-").is_err());
    assert!(parser.parse("-1234").is_err());
    assert!(parser.parse("123-ME").is_err());
    assert!(parser.parse("").is_err());
  }

  #[test]
  fn test_commit_message_extraction_strict() {
    let parser = JiraTicketParser::new_strict();

    assert_eq!(
      parser.extract_from_commit_message("ME-1234: Fix bug in parser"),
      Some("ME-1234".to_string())
    );
    assert_eq!(
      parser.extract_from_commit_message("me-1234: Fix bug in parser"),
      None // Strict mode doesn't accept lowercase
    );
  }

  #[test]
  fn test_commit_message_extraction_flexible() {
    let parser = JiraTicketParser::new_flexible();

    assert_eq!(
      parser.extract_from_commit_message("ME-1234: Fix bug in parser"),
      Some("ME-1234".to_string())
    );
    assert_eq!(
      parser.extract_from_commit_message("me-1234: Fix bug in parser"),
      Some("ME-1234".to_string())
    );
    assert_eq!(
      parser.extract_from_commit_message("ME1234: Fix bug in parser"),
      Some("ME-1234".to_string())
    );
  }

  #[test]
  fn test_is_valid() {
    let parser = JiraTicketParser::new_default();

    assert!(parser.is_valid("ME-1234"));
    assert!(parser.is_valid("me1234"));
    assert!(!parser.is_valid("invalid"));
  }

  #[test]
  fn test_long_project_names() {
    let parser = JiraTicketParser::new_default();

    assert_eq!(parser.parse("VERYLONGPROJECT-123").unwrap(), "VERYLONGPROJECT-123");
    assert_eq!(parser.parse("verylongproject123").unwrap(), "VERYLONGPROJECT-123");
  }

  #[test]
  fn test_leading_zeros() {
    let parser = JiraTicketParser::new_default();

    assert_eq!(parser.parse("ME-0123").unwrap(), "ME-0123");
    assert_eq!(parser.parse("me0123").unwrap(), "ME-0123");
  }
}
