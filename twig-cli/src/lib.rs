//! # Twig CLI Library
//!
//! Core library modules for the twig command-line tool, providing functionality
//! for Git branch dependency management, tree visualization, and developer
//! workflows.

pub mod auto_dependency_discovery;
pub mod cli;
pub mod complete;
pub mod completion;
pub mod consts;
pub mod diagnostics;
pub mod fixup;
pub mod git;
pub mod git_commands;
pub mod plugin;
pub mod self_update;
pub mod user_defined_dependency_resolver;
pub mod utils;

pub use twig_core::creds;
