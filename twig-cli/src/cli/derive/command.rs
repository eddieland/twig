//! # Command Trait
//!
//! Defines the standard interface for derive-based CLI commands.
//! This trait provides a consistent pattern for command implementation
//! and execution.

use anyhow::Result;
use clap::Parser;

/// Standard interface for derive-based CLI commands
///
/// This trait should be implemented by all derive-based command structs
/// to ensure a consistent pattern across the codebase.
pub trait DeriveCommand: Parser {
  /// Execute the command with the parsed arguments
  fn execute(self) -> Result<()>;
}
