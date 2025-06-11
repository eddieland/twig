//! # Twig Core Library
//!
//! Core library for twig plugins providing configuration structures, state
//! models, and utility functions for plugin developers. This crate enables
//! plugins to access twig's configuration and state in a read-only manner while
//! maintaining their own separate state.

pub mod config;
pub mod git;
pub mod output;
pub mod state;
pub mod utils;

// Re-export main types for plugin developers
pub use config::{ConfigDirs, get_config_dirs};
pub use git::{current_branch, current_repository, detect_repository, in_git_repository};
pub use output::{ColorMode, format_repo_path, print_error, print_info, print_success, print_warning};
pub use state::{Registry, RepoState, Repository};

/// Plugin-specific utilities
pub mod plugin {
  use std::path::PathBuf;

  use anyhow::Result;

  pub use super::config::get_config_dirs;
  pub use super::git::{current_branch, current_repository};

  /// Get the current working directory as a repository path
  pub fn current_working_repo() -> Result<Option<PathBuf>> {
    current_repository()
  }

  /// Check if we're currently in a git repository
  pub fn in_git_repository() -> bool {
    super::git::in_git_repository()
  }

  /// Get plugin-specific config directory
  pub fn plugin_config_dir(plugin_name: &str) -> Result<PathBuf> {
    let config_dirs = get_config_dirs()?;
    Ok(config_dirs.config_dir().join("plugins").join(plugin_name))
  }

  /// Get plugin-specific data directory
  pub fn plugin_data_dir(plugin_name: &str) -> Result<PathBuf> {
    let config_dirs = get_config_dirs()?;
    Ok(config_dirs.data_dir().join("plugins").join(plugin_name))
  }
}
