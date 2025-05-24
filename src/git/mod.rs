use anyhow::{Context, Result};
use git2::{FetchOptions, Repository as Git2Repository};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::Registry;

/// Add a repository to the registry
pub fn add_repository<P: AsRef<Path>>(path: P) -> Result<()> {
  let config_dirs = crate::config::ConfigDirs::new()?;
  let mut registry = Registry::load(&config_dirs)?;

  registry.add(path)?;
  registry.save(&config_dirs)?;

  Ok(())
}

/// Remove a repository from the registry
pub fn remove_repository<P: AsRef<Path>>(path: P) -> Result<()> {
  let config_dirs = crate::config::ConfigDirs::new()?;
  let mut registry = Registry::load(&config_dirs)?;

  registry.remove(path)?;
  registry.save(&config_dirs)?;

  Ok(())
}

/// List all repositories in the registry
pub fn list_repositories() -> Result<()> {
  use crate::utils::output::{
    format_command, format_repo_name, format_repo_path, format_timestamp, print_header, print_info, print_warning,
  };

  let config_dirs = crate::config::ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    print_warning("No repositories in registry.");
    print_info(&format!("Add one with {}", format_command("twig git add <path>")));
    return Ok(());
  }

  print_header("Tracked Repositories");
  for repo in repos {
    let last_fetch = repo.last_fetch.as_deref().unwrap_or("never");
    println!("  {} ({})", format_repo_name(&repo.name), format_repo_path(&repo.path));
    println!("    Last fetch: {}", format_timestamp(last_fetch));
  }

  Ok(())
}

/// Fetch updates for a repository
pub fn fetch_repository<P: AsRef<Path>>(path: P, all: bool) -> Result<()> {
  let path = path.as_ref();
  let repo = Git2Repository::open(path).context(format!("Failed to open git repository at {}", path.display()))?;

  let mut fetch_options = FetchOptions::new();

  if all {
    // Fetch all remotes
    let remotes = repo.remotes()?;
    for i in 0..remotes.len() {
      let remote_name = remotes.get(i).unwrap();
      use crate::utils::output::print_info;
      print_info(&format!("Fetching remote: {}", remote_name));

      let mut remote = repo.find_remote(remote_name)?;
      remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .context(format!("Failed to fetch from remote '{}'", remote_name))?;
    }
  } else {
    // Just fetch origin
    use crate::utils::output::print_info;
    print_info("Fetching remote: origin");
    let mut remote = repo.find_remote("origin")?;
    remote
      .fetch(&[] as &[&str], Some(&mut fetch_options), None)
      .context("Failed to fetch from remote 'origin'")?;
  }

  // Update the last fetch time in the registry
  let config_dirs = crate::config::ConfigDirs::new()?;
  let mut registry = Registry::load(&config_dirs)?;

  let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
  let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
    .unwrap()
    .to_rfc3339();

  registry
    .update_fetch_time(path, time_str)
    .context("Failed to update fetch time in registry")?;
  registry.save(&config_dirs)?;

  use crate::utils::output::{format_repo_path, print_success};
  print_success(&format!(
    "Successfully fetched repository at {}",
    format_repo_path(&path.display().to_string())
  ));
  Ok(())
}

/// Fetch updates for all repositories in the registry
pub fn fetch_all_repositories() -> Result<()> {
  let config_dirs = crate::config::ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    use crate::utils::output::{format_command, print_info, print_warning};
    print_warning("No repositories in registry.");
    print_info(&format!("Add one with {}", format_command("twig git add <path>")));
    return Ok(());
  }

  for repo in repos {
    use crate::utils::output::{format_repo_name, format_repo_path, print_error, print_info};
    print_info(&format!(
      "Fetching repository: {} ({})",
      format_repo_name(&repo.name),
      format_repo_path(&repo.path)
    ));
    if let Err(e) = fetch_repository(&repo.path, true) {
      print_error(&format!(
        "Error fetching repository {}: {}",
        format_repo_path(&repo.path),
        e
      ));
    }
  }

  Ok(())
}

/// Detect the current working directory repository
pub fn detect_current_repository() -> Result<PathBuf> {
  let current_dir = std::env::current_dir().context("Failed to get current directory")?;

  // Try to find a git repository in the current directory or any parent
  let mut path = current_dir.clone();
  loop {
    let git_dir = path.join(".git");
    if git_dir.exists() && git_dir.is_dir() {
      return Ok(path);
    }

    if !path.pop() {
      break;
    }
  }

  Err(anyhow::anyhow!(
    "No git repository found in current directory or any parent"
  ))
}
