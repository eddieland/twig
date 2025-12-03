//! # Plugin Discovery and Execution
//!
//! Implements the plugin discovery system that allows twig to execute external
//! plugins following the kubectl/Docker-inspired plugin model.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use twig_core::output::ColorMode;

/// Execute a plugin with the given name and arguments
pub fn execute_plugin(plugin_name: &str, args: Vec<String>, verbosity: u8, colors: ColorMode) -> Result<()> {
  let plugin_binary = format!("twig-{plugin_name}");

  // Check if plugin exists in PATH
  if !plugin_exists(&plugin_binary)? {
    return Err(anyhow::anyhow!(
      "Unknown command '{plugin_name}'. No plugin 'twig-{plugin_name}' found in PATH.\n\n\
             To install plugins, place executable files named 'twig-<command>' in your PATH."
    ));
  }

  // Set up environment variables
  let config_dirs = twig_core::get_config_dirs()?;
  let current_repo = twig_core::detect_repository();
  let current_branch = twig_core::current_branch().unwrap_or(None);

  let mut cmd = Command::new(&plugin_binary);
  cmd
    .args(args)
    .env("TWIG_CONFIG_DIR", config_dirs.config_dir())
    .env("TWIG_DATA_DIR", config_dirs.data_dir())
    .env("TWIG_COLORS", color_mode_env(colors))
    .env("TWIG_VERSION", env!("CARGO_PKG_VERSION"))
    .env("TWIG_VERBOSITY", verbosity.to_string())
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

  if let Some(repo) = current_repo {
    cmd.env("TWIG_CURRENT_REPO", repo.display().to_string());
  }

  if let Some(branch) = current_branch {
    cmd.env("TWIG_CURRENT_BRANCH", branch);
  }

  let status = cmd
    .status()
    .with_context(|| format!("Failed to execute plugin '{plugin_binary}'"))?;

  std::process::exit(status.code().unwrap_or(1));
}

/// Determine if a plugin binary is available in the current PATH
pub fn plugin_is_available(plugin_name: &str) -> Result<bool> {
  let plugin_binary = format!("twig-{plugin_name}");
  plugin_exists(&plugin_binary)
}

/// Check if a plugin exists in PATH
fn plugin_exists(plugin_name: &str) -> Result<bool> {
  // Try 'which' command (Unix-like systems)
  if let Ok(output) = Command::new("which").arg(plugin_name).output() {
    return Ok(output.status.success());
  }

  // Try 'where' command (Windows)
  if let Ok(output) = Command::new("where").arg(plugin_name).output() {
    return Ok(output.status.success());
  }

  // Fallback: try to execute the command with --help
  if let Ok(output) = Command::new(plugin_name).arg("--help").output() {
    return Ok(output.status.success());
  }

  Ok(false)
}

fn color_mode_env(mode: ColorMode) -> &'static str {
  match mode {
    ColorMode::Yes => "yes",
    ColorMode::No => "no",
    ColorMode::Auto => "auto",
  }
}

/// Metadata describing a discovered plugin binary.
#[derive(Debug, Clone)]
pub struct PluginInfo {
  /// Canonical plugin name without the `twig-` prefix.
  pub name: String,
  /// Ordered list of plugin locations in PATH order (primary first).
  pub paths: Vec<PathBuf>,
  /// File size in bytes for the primary plugin location, if available.
  pub size_in_bytes: Option<u64>,
}

/// List available plugins in PATH with basic metadata.
pub fn list_available_plugins() -> Result<Vec<PluginInfo>> {
  let path_var = env::var("PATH").unwrap_or_default();
  list_available_plugins_from_path(&path_var)
}

fn list_available_plugins_from_path(path_var: &str) -> Result<Vec<PluginInfo>> {
  let mut plugins: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();

  for path in env::split_paths(path_var) {
    if !path.exists() {
      continue;
    }

    if let Ok(entries) = std::fs::read_dir(path) {
      for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Check if it's a twig plugin
        if file_name_str.starts_with("twig-") {
          let plugin_name = file_name_str.strip_prefix("twig-").unwrap();

          // Remove file extension on Windows
          let plugin_name = if cfg!(windows) && plugin_name.ends_with(".exe") {
            plugin_name.strip_suffix(".exe").unwrap()
          } else {
            plugin_name
          };

          let entry_path = entry.path();
          let plugin_paths = plugins.entry(plugin_name.to_string()).or_default();

          if !plugin_paths.contains(&entry_path) {
            plugin_paths.push(entry_path);
          }
        }
      }
    }
  }

  let mut plugin_info: Vec<PluginInfo> = Vec::new();

  for (name, paths) in plugins {
    if paths.is_empty() {
      continue;
    }

    let size_in_bytes = std::fs::metadata(&paths[0]).map(|metadata| metadata.len()).ok();

    plugin_info.push(PluginInfo {
      name,
      paths,
      size_in_bytes,
    });
  }

  Ok(plugin_info)
}

/// Generate suggestions for unknown commands
#[allow(dead_code)]
pub fn suggest_similar_commands(unknown_command: &str, available_plugins: &[String]) -> Vec<String> {
  let mut suggestions = Vec::new();

  // Built-in commands that might be similar
  let builtin_commands = [
    "branch",
    "cascade",
    "commit",
    "creds",
    "dashboard",
    "git",
    "github",
    "jira",
    "self",
    "rebase",
    "switch",
    "sync",
    "tree",
    "worktree",
  ];

  // Combine built-in commands and plugins
  let mut all_commands: Vec<&str> = builtin_commands.to_vec();
  for plugin in available_plugins {
    all_commands.push(plugin.as_str());
  }

  // Simple string distance matching
  for command in all_commands.iter() {
    if levenshtein_distance(unknown_command, command) <= 2 {
      suggestions.push(command.to_string());
    }
  }

  // If no close matches, suggest commands that start with the same letter
  if suggestions.is_empty() {
    let first_char = unknown_command
      .chars()
      .next()
      .unwrap_or('\0')
      .to_lowercase()
      .next()
      .unwrap_or('\0');
    for command in all_commands.iter() {
      if command
        .chars()
        .next()
        .unwrap_or('\0')
        .to_lowercase()
        .next()
        .unwrap_or('\0')
        == first_char
      {
        suggestions.push(command.to_string());
      }
    }
  }

  suggestions.sort();
  suggestions.dedup();
  suggestions.truncate(5); // Limit to 5 suggestions
  suggestions
}

/// Calculate Levenshtein distance between two strings
#[allow(dead_code)]
#[allow(clippy::needless_range_loop)] // iterator approach hurts readability
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
  let len1 = s1.len();
  let len2 = s2.len();

  if len1 == 0 {
    return len2;
  }
  if len2 == 0 {
    return len1;
  }

  let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

  for i in 0..=len1 {
    matrix[i][0] = i;
  }
  for j in 0..=len2 {
    matrix[0][j] = j;
  }

  let s1_chars: Vec<char> = s1.chars().collect();
  let s2_chars: Vec<char> = s2.chars().collect();

  for i in 1..=len1 {
    for j in 1..=len2 {
      let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
      matrix[i][j] = std::cmp::min(
        std::cmp::min(
          matrix[i - 1][j] + 1, // deletion
          matrix[i][j - 1] + 1, // insertion
        ),
        matrix[i - 1][j - 1] + cost, // substitution
      );
    }
  }

  matrix[len1][len2]
}

#[cfg(test)]
mod tests {
  use std::fs;

  use tempfile::TempDir;

  use super::*;

  #[test]
  fn test_levenshtein_distance() {
    assert_eq!(levenshtein_distance("", ""), 0);
    assert_eq!(levenshtein_distance("abc", "abc"), 0);
    assert_eq!(levenshtein_distance("abc", "ab"), 1);
    assert_eq!(levenshtein_distance("abc", "def"), 3);
    assert_eq!(levenshtein_distance("branch", "brach"), 1);
    assert_eq!(levenshtein_distance("deploy", "deploi"), 1);
  }

  #[test]
  fn test_suggest_similar_commands() {
    let plugins = vec!["deploy".to_string(), "backup".to_string()];

    let suggestions = suggest_similar_commands("deploi", &plugins);
    assert!(suggestions.contains(&"deploy".to_string()));

    let suggestions = suggest_similar_commands("brach", &plugins);
    assert!(suggestions.contains(&"branch".to_string()));
  }

  #[test]
  fn list_available_plugins_collects_paths_and_sizes() {
    let first_dir = TempDir::new().expect("failed to create temp dir");
    let second_dir = TempDir::new().expect("failed to create temp dir");

    let primary_plugin = first_dir.path().join("twig-example");
    fs::write(&primary_plugin, b"#!/bin/sh\necho primary\n").unwrap();

    let duplicate_plugin = second_dir.path().join("twig-example");
    fs::write(&duplicate_plugin, b"#!/bin/sh\necho duplicate\n").unwrap();

    let secondary_plugin = second_dir.path().join("twig-another");
    fs::write(&secondary_plugin, b"#!/bin/sh\necho another\n").unwrap();

    let custom_path = std::env::join_paths([first_dir.path(), second_dir.path()])
      .expect("failed to construct custom PATH")
      .into_string()
      .expect("temporary plugin paths should be valid UTF-8");

    let plugins = list_available_plugins_from_path(&custom_path).expect("listing plugins failed");

    let example = plugins
      .iter()
      .find(|plugin| plugin.name == "example")
      .expect("example plugin missing");

    assert_eq!(example.paths, vec![primary_plugin.clone(), duplicate_plugin]);
    assert_eq!(
      example.size_in_bytes,
      fs::metadata(&primary_plugin).ok().map(|metadata| metadata.len())
    );

    let another = plugins
      .iter()
      .find(|plugin| plugin.name == "another")
      .expect("another plugin missing");

    assert_eq!(another.paths, vec![secondary_plugin.clone()]);
    assert_eq!(
      another.size_in_bytes,
      fs::metadata(&secondary_plugin).ok().map(|metadata| metadata.len())
    );
  }
}
