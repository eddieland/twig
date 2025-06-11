//! Config Init command
//!
//! This module provides the `init` command to initialize Twig's configuration
//! directories.

use anyhow::Result;
use twig_core::ConfigDirs;

/// Initialize the configuration directories
pub fn handle_init_command() -> Result<()> {
  use twig_core::output::{format_repo_path, print_success};

  let config_dirs = ConfigDirs::new()?;
  config_dirs.init()?;

  print_success("Initialized twig configuration directories:");
  println!(
    "  Config: {}",
    format_repo_path(&config_dirs.config_dir.display().to_string())
  );
  println!(
    "  Data: {}",
    format_repo_path(&config_dirs.data_dir.display().to_string())
  );

  Ok(())
}
