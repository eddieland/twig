use std::fs;
use std::path::Path;

/// Get candidate filenames for a plugin, considering platform-specific
/// executable extensions.
///
/// On non-Windows platforms, this simply returns the plugin name as-is.
#[cfg(unix)]
pub fn candidate_filenames(plugin_name: &str) -> Vec<String> {
  vec![plugin_name.to_string()]
}

/// Check if a given path is an executable file.
#[cfg(unix)]
pub fn is_executable(path: &Path) -> bool {
  let Ok(metadata) = fs::metadata(path) else {
    return false;
  };

  if !metadata.is_file() {
    return false;
  }

  use std::os::unix::fs::PermissionsExt;
  metadata.permissions().mode() & 0o111 != 0
}

/// Get candidate filenames for a plugin, considering platform-specific
/// executable extensions.
///
/// This provides the Windows-specific behavior of appending `.exe` if not
/// already present.
#[cfg(not(unix))]
pub fn candidate_filenames(plugin_name: &str) -> Vec<String> {
  let mut names = vec![plugin_name.to_string()];

  if !plugin_name.to_lowercase().ends_with(".exe") {
    names.push(format!("{plugin_name}.exe"));
  }

  names
}

/// Check if a given path is an executable file.
#[cfg(not(unix))]
pub fn is_executable(path: &Path) -> bool {
  fs::metadata(path).map(|meta| meta.is_file()).unwrap_or(false)
}
