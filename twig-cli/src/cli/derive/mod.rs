//! # Derive-based CLI Implementation
//!
//! This module contains the derive-based CLI implementation for the twig tool.
//! It uses the clap derive feature to define commands as structs, reducing
//! boilerplate and improving maintainability.

// Export the command trait
pub mod command;
pub use command::DeriveCommand;

// Export submodules
pub mod branch;
pub mod completion;
pub mod creds;
pub mod dashboard;
pub mod diagnostics;
pub mod git;
pub mod github;
pub mod init;
pub mod jira;
pub mod panic;
pub mod switch;
pub mod sync;
pub mod tree;
pub mod view;
pub mod worktree;
