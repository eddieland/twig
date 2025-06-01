//! # Diagnostics Command
//!
//! Derive-based implementation of the diagnostics command for running
//! system diagnostics.

use anyhow::Result;

use crate::diagnostics::run_diagnostics;

pub(crate) fn handle_diagnostics_command() -> Result<()> {
  run_diagnostics()
}
