//! # Twig CLI Library
//!
//! Core library modules for the twig command-line tool, providing functionality
//! for Git branch dependency management, tree visualization, and developer
//! workflows.

pub mod auto_dependency_discovery;
pub mod cli;
pub mod completion;
pub mod consts;
pub mod diagnostics;
pub mod fixup;
pub mod git;
pub mod plugin;
pub mod utils;
