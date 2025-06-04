//! Platform-specific utilities
//!
//! This module provides platform-specific utilities for command execution
//! and other platform-dependent operations.

/// Platform-specific Git executable name
#[cfg(windows)]
#[cfg_attr(not(windows), allow(dead_code))]
pub const GIT_EXECUTABLE: &str = "git.exe";

/// Platform-specific Git executable name
#[cfg(not(windows))]
#[cfg_attr(windows, allow(dead_code))]
pub const GIT_EXECUTABLE: &str = "git";

/// Convert a path to use the correct path separators for the current platform
///
/// On Windows, this converts forward slashes to backslashes.
/// On other platforms, this is a no-op.
#[allow(dead_code)]
pub fn normalize_path(path: &str) -> String {
  #[cfg(windows)]
  {
    path.replace('/', "\\")
  }
  #[cfg(not(windows))]
  {
    path.to_string()
  }
}
