use std::fs;
use std::path::Path;

#[cfg(windows)]
pub fn candidate_filenames(plugin_name: &str) -> Vec<String> {
  let mut names = vec![plugin_name.to_string()];

  if !plugin_name.to_lowercase().ends_with(".exe") {
    names.push(format!("{plugin_name}.exe"));
  }

  names
}

#[cfg(not(windows))]
pub fn candidate_filenames(plugin_name: &str) -> Vec<String> {
  vec![plugin_name.to_string()]
}

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

#[cfg(not(unix))]
pub fn is_executable(path: &Path) -> bool {
  fs::metadata(path).map(|meta| meta.is_file()).unwrap_or(false)
}
