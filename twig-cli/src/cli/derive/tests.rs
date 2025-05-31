//! # Tests for Derive-based CLI Implementation
//!
//! This module contains tests for the derive-based CLI implementation.

#[cfg(test)]
mod tests {
  use clap::CommandFactory;

  use crate::cli::derive::{InitCommand, PanicCommand, TreeCommand};

  #[test]
  fn test_init_command() {
    // Verify that the command can be built without errors
    let cmd = InitCommand::command();
    // The command name is "twig-cli" by default, we'll check the about text instead
    assert!(cmd.get_about().is_some());
  }

  #[test]
  fn test_panic_command() {
    // Verify that the command can be built without errors
    let cmd = PanicCommand::command();
    // The command name is "twig-cli" by default, we'll check the about text instead
    assert!(cmd.get_about().is_some());
    assert!(cmd.is_hide_set());
  }

  #[test]
  fn test_tree_command() {
    // Verify that the command can be built without errors
    let cmd = TreeCommand::command();
    // The command name is "twig-cli" by default, we'll check the about text instead
    assert!(cmd.get_about().is_some());

    // Verify that the command has the expected arguments
    let args: Vec<_> = cmd.get_arguments().collect();
    assert!(args.iter().any(|arg| arg.get_id() == "repo"));
    assert!(args.iter().any(|arg| arg.get_id() == "max_depth"));
    assert!(args.iter().any(|arg| arg.get_id() == "no_color"));
  }
}
