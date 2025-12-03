//! Plugin-facing helpers for discovering Twig context.
//!
//! Plugins normally receive context via environment variables set by the Twig
//! CLI. The helpers in this module make it possible to reconstruct that
//! context when those variables are missing (for example, when a plugin is
//! executed directly during development).

use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use git2::Repository;

use crate::config::ConfigDirs;
pub use crate::config::get_config_dirs;
use crate::git::detect_repository;
pub use crate::git::{checkout_branch, current_branch, detect_repository_from_path, get_repository, in_git_repository};
use crate::output::ColorMode;

/// Resolved context for a plugin invocation.
#[derive(Debug, Clone)]
pub struct PluginContext {
  /// Twig configuration, data, and cache directories.
  pub config_dirs: ConfigDirs,
  /// Repository path provided by Twig or discovered from the current directory.
  pub current_repo: Option<PathBuf>,
  /// Current branch provided by Twig or inferred from the repository.
  pub current_branch: Option<String>,
  /// Color preference propagated from Twig when available.
  pub colors: ColorMode,
  /// Verbosity level propagated from Twig when available.
  pub verbosity: u8,
  /// Version of the Twig binary that invoked the plugin, if known.
  pub version: Option<String>,
}

impl PluginContext {
  /// Load the plugin context from environment variables, falling back to
  /// auto-discovery when invoked outside the Twig CLI.
  pub fn discover() -> Result<Self> {
    let config_dirs = config_dirs_from_env_or_default()?;
    let current_repo = env::var_os("TWIG_CURRENT_REPO")
      .map(PathBuf::from)
      .or_else(detect_repository);
    let current_branch = branch_from_env_or_repo(current_repo.as_deref());

    let colors = match env::var("TWIG_COLORS") {
      Ok(value) if value.eq_ignore_ascii_case("yes") => ColorMode::Yes,
      Ok(value) if value.eq_ignore_ascii_case("no") => ColorMode::No,
      _ => ColorMode::Auto,
    };

    let verbosity = env::var("TWIG_VERBOSITY")
      .ok()
      .and_then(|value| value.parse::<u8>().ok())
      .unwrap_or(0);

    let version = env::var("TWIG_VERSION").ok();

    Ok(Self {
      config_dirs,
      current_repo,
      current_branch,
      colors,
      verbosity,
      version,
    })
  }

  /// Compute the plugin-specific config directory.
  pub fn plugin_config_dir<P: AsRef<Path>>(&self, plugin_name: P) -> PathBuf {
    self.config_dirs.config_dir().join("plugins").join(plugin_name.as_ref())
  }

  /// Compute the plugin-specific data directory.
  pub fn plugin_data_dir<P: AsRef<Path>>(&self, plugin_name: P) -> PathBuf {
    self.config_dirs.data_dir().join("plugins").join(plugin_name.as_ref())
  }
}

/// Get plugin-specific config directory using environment overrides when
/// present.
pub fn plugin_config_dir(plugin_name: &str) -> Result<PathBuf> {
  let config_dirs = config_dirs_from_env_or_default()?;
  Ok(config_dirs.config_dir().join("plugins").join(plugin_name))
}

/// Get plugin-specific data directory using environment overrides when present.
pub fn plugin_data_dir(plugin_name: &str) -> Result<PathBuf> {
  let config_dirs = config_dirs_from_env_or_default()?;
  Ok(config_dirs.data_dir().join("plugins").join(plugin_name))
}

fn config_dirs_from_env_or_default() -> Result<ConfigDirs> {
  let defaults = get_config_dirs()?;

  let config_dir = env::var_os("TWIG_CONFIG_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| defaults.config_dir().clone());

  let data_dir = env::var_os("TWIG_DATA_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| defaults.data_dir().clone());

  Ok(ConfigDirs {
    config_dir,
    data_dir,
    cache_dir: defaults.cache_dir().cloned(),
  })
}

fn branch_from_env_or_repo(repo_path: Option<&Path>) -> Option<String> {
  if let Ok(branch) = env::var("TWIG_CURRENT_BRANCH")
    && !branch.is_empty()
  {
    return Some(branch);
  }

  if let Some(repo_path) = repo_path
    && let Ok(repo) = Repository::open(repo_path)
    && let Ok(head) = repo.head()
    && let Some(name) = head.shorthand()
  {
    return Some(name.to_string());
  }

  current_branch().unwrap_or(None)
}

#[cfg(test)]
mod tests {
  use std::{env, fs};

  use git2::Repository as GitRepository;
  use tempfile::TempDir;
  use twig_test_utils::env::{EnvTestGuard, EnvVarGuard};

  use super::*;

  fn init_repo_with_commit(dir: &TempDir) -> String {
    let repo = GitRepository::init(dir.path()).expect("failed to init repo");
    let sig = git2::Signature::now("Tester", "tester@example.com").expect("signature");
    let tree_id = {
      let mut index = repo.index().expect("index");
      index.write_tree().expect("write tree")
    };
    let tree = repo.find_tree(tree_id).expect("tree");
    repo
      .commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
      .expect("commit");
    repo.head().expect("head").shorthand().unwrap_or_default().to_string()
  }

  #[test]
  fn discover_prefers_environment_values() {
    let _xdg = EnvTestGuard::new();
    let temp_config = TempDir::new().unwrap();
    let temp_data = TempDir::new().unwrap();

    let config_guard = EnvVarGuard::new("TWIG_CONFIG_DIR");
    let data_guard = EnvVarGuard::new("TWIG_DATA_DIR");
    let repo_guard = EnvVarGuard::new("TWIG_CURRENT_REPO");
    let branch_guard = EnvVarGuard::new("TWIG_CURRENT_BRANCH");
    let verbosity_guard = EnvVarGuard::new("TWIG_VERBOSITY");
    let colors_guard = EnvVarGuard::new("TWIG_COLORS");
    let version_guard = EnvVarGuard::new("TWIG_VERSION");

    config_guard.set(temp_config.path());
    data_guard.set(temp_data.path());
    repo_guard.set("/tmp/repo-from-env");
    branch_guard.set("feature/env-branch");
    verbosity_guard.set("2");
    colors_guard.set("no");
    version_guard.set("0.0.0-env");

    let context = PluginContext::discover().expect("context");

    assert_eq!(context.config_dirs.config_dir(), &temp_config.path().to_path_buf());
    assert_eq!(context.config_dirs.data_dir(), &temp_data.path().to_path_buf());
    assert_eq!(context.current_repo, Some(PathBuf::from("/tmp/repo-from-env")));
    assert_eq!(context.current_branch, Some("feature/env-branch".to_string()));
    assert_eq!(context.verbosity, 2);
    assert_eq!(context.colors, ColorMode::No);
    assert_eq!(context.version.as_deref(), Some("0.0.0-env"));
  }

  #[test]
  fn discover_falls_back_to_defaults() {
    let _xdg = EnvTestGuard::new();
    EnvVarGuard::new("TWIG_CONFIG_DIR").remove();
    EnvVarGuard::new("TWIG_DATA_DIR").remove();
    EnvVarGuard::new("TWIG_CURRENT_REPO").remove();
    EnvVarGuard::new("TWIG_CURRENT_BRANCH").remove();
    EnvVarGuard::new("TWIG_VERBOSITY").remove();
    EnvVarGuard::new("TWIG_COLORS").remove();
    EnvVarGuard::new("TWIG_VERSION").remove();

    let repo_dir = TempDir::new().unwrap();
    let branch_name = init_repo_with_commit(&repo_dir);

    let original_dir = env::current_dir().expect("cwd");
    env::set_current_dir(repo_dir.path()).expect("chdir");

    let canonical_repo_path = fs::canonicalize(repo_dir.path()).expect("canonical repo path");
    let expected_dirs = get_config_dirs().expect("config dirs");
    let context = PluginContext::discover().expect("context");

    assert_eq!(context.config_dirs.config_dir(), expected_dirs.config_dir());
    assert_eq!(context.config_dirs.data_dir(), expected_dirs.data_dir());

    let Some(current_repo) = &context.current_repo else {
      panic!("expected a discovered repository path");
    };
    let canonical_current_repo = fs::canonicalize(current_repo).expect("canonical current repo path");
    assert_eq!(canonical_current_repo, canonical_repo_path);
    assert_eq!(context.current_branch, Some(branch_name));
    assert_eq!(context.colors, ColorMode::Auto);
    assert_eq!(context.verbosity, 0);
    assert!(context.version.is_none());

    env::set_current_dir(original_dir).expect("restore dir");
  }
}
