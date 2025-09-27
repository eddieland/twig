//! # Twig CLI Library
//!
//! Core library modules for the twig command-line tool, providing functionality
//! for Git branch dependency management, tree visualization, and developer
//! workflows.

pub mod auto_dependency_discovery;
pub mod cli;
pub mod clients;
pub mod completion;
pub mod consts;
pub mod creds;
pub mod diagnostics;
pub mod enhanced_errors;
pub mod fixup;
pub mod git;
pub mod plugin;
pub mod user_defined_dependency_resolver;
pub mod user_experience;
pub mod utils;

#[cfg(test)]
mod test_enhanced_features;
