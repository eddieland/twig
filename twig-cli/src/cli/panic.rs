//! # Panic Command
//!
//! Derive-based implementation of the panic command for testing the panic
//! handler.

use anyhow::Result;

pub(crate) fn handle_panic_command() -> Result<()> {
  panic!("This is an intentional test panic to verify no-worries integration");
}
