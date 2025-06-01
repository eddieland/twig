//! # Init Command
//!
//! Derive-based implementation of the init command for initializing twig
//! configuration.

use anyhow::Result;

pub(crate) fn handle_init_command() -> Result<()> {
  crate::config::init()
}
