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
