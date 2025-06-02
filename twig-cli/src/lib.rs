//! # Twig CLI Library
//!
//! Core library modules for the twig command-line tool, providing functionality
//! for Git branch dependency management, tree visualization, and developer
//! workflows.

pub mod auto_dependency_discovery;
pub mod cli;
pub mod clients;
pub mod completion;
pub mod config;
pub mod consts;
pub mod creds;
pub mod diagnostics;
pub mod git;
pub mod repo_state;
pub mod state;
pub mod tree_renderer;
pub mod user_defined_dependency_resolver;
pub mod utils;
