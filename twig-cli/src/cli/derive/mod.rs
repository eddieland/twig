//! # Derive-based CLI Implementation
//!
//! This module contains the derive-based CLI implementation for the twig tool.
//! It uses the clap derive feature to define commands as structs, reducing
//! boilerplate and improving maintainability.

// Export the command trait
pub mod command;
pub use command::DeriveCommand;

// Export submodules
pub mod init;
pub mod panic;
pub mod switch;
pub mod tree;
