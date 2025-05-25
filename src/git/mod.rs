use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use git2::{BranchType, FetchOptions, Repository as Git2Repository};
use tokio::{task, time};

use crate::state::Registry;
use crate::utils::output::{format_repo_name, format_repo_path, print_error, print_info, print_success, print_warning};

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
      print_info(&format!("Fetching remote: {remote_name}"));

      let mut remote = repo.find_remote(remote_name)?;
      remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .context(format!("Failed to fetch from remote '{remote_name}'"))?;
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
    print_warning("No repositories in registry.");
    print_info(&format!(
      "Add one with {}",
      crate::utils::output::format_command("twig git add <path>")
    ));
    return Ok(());
  }

  print_info(&format!("Fetching updates for {} repositories", repos.len()));

  // Create a tokio runtime for parallel execution
  let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

  rt.block_on(async {
    let mut handles = Vec::new();

    // Launch tasks for each repository
    for repo in repos {
      let repo_path = repo.path.clone();
      let repo_name = repo.name.clone();

      let handle = task::spawn(async move {
        print_info(&format!(
          "Fetching repository: {} ({})",
          format_repo_name(&repo_name),
          format_repo_path(&repo_path)
        ));

        let result = fetch_repository(&repo_path, true);
        (repo_name, repo_path, result)
      });

      handles.push(handle);

      // Small delay to avoid overwhelming the system
      time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all tasks to complete
    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
      match handle.await {
        Ok((_name, _path, Ok(()))) => {
          success_count += 1;
        }
        Ok((name, path, Err(e))) => {
          print_error(&format!(
            "Error fetching repository {} ({}): {}",
            format_repo_name(&name),
            format_repo_path(&path),
            e
          ));
          failure_count += 1;
        }
        Err(e) => {
          print_error(&format!("Task panicked: {e}"));
          failure_count += 1;
        }
      }
    }

    // Print summary
    print_info("Fetch operation complete");
    print_info(&format!("Successful: {success_count}"));

    if failure_count > 0 {
      print_warning(&format!("Failed: {failure_count}"));
    }
  });

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

/// Execute a command in a repository
pub fn execute_repository<P: AsRef<Path>>(path: P, command: &str) -> Result<()> {
  let path = path.as_ref();

  print_info(&format!(
    "Executing in repository: {}",
    format_repo_path(&path.display().to_string())
  ));

  // Split the command into program and arguments
  let mut parts = command.split_whitespace();
  let program = parts.next().unwrap_or("git");
  let args: Vec<&str> = parts.collect();

  // Execute the command
  let output = Command::new(program)
    .args(&args)
    .current_dir(path)
    .output()
    .context(format!("Failed to execute command: {command}"))?;

  // Print the output
  if !output.stdout.is_empty() {
    println!("{}", String::from_utf8_lossy(&output.stdout));
  }

  if !output.stderr.is_empty() {
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
  }

  if output.status.success() {
    print_success(&format!(
      "Command executed successfully in {}",
      format_repo_path(&path.display().to_string())
    ));
    Ok(())
  } else {
    print_error(&format!(
      "Command failed in {} with exit code: {}",
      format_repo_path(&path.display().to_string()),
      output.status
    ));
    Err(anyhow::anyhow!("Command execution failed"))
  }
}

/// Execute a command in all repositories
pub fn execute_all_repositories(command: &str) -> Result<()> {
  let config_dirs = crate::config::ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    print_warning("No repositories in registry.");
    print_info(&format!(
      "Add one with {}",
      crate::utils::output::format_command("twig git add <path>")
    ));
    return Ok(());
  }

  print_info(&format!(
    "Executing command in {} repositories: {}",
    repos.len(),
    command
  ));

  // Create a tokio runtime for parallel execution
  let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

  rt.block_on(async {
    let mut handles = Vec::new();

    // Launch tasks for each repository
    for repo in repos {
      let repo_path = repo.path.clone();
      let cmd = command.to_string();

      let handle = task::spawn(async move {
        let result = execute_repository(&repo_path, &cmd);
        (repo_path, result)
      });

      handles.push(handle);

      // Small delay to avoid overwhelming the system
      time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all tasks to complete and collect results
    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
      match handle.await {
        Ok((_path, Ok(()))) => {
          success_count += 1;
        }
        Ok((_path, Err(_e))) => {
          failure_count += 1;
        }
        Err(e) => {
          print_error(&format!("Task panicked: {e}"));
          failure_count += 1;
        }
      }
    }

    // Print summary
    print_info("Command execution complete");
    print_info(&format!("Successful: {success_count}"));

    if failure_count > 0 {
      print_warning(&format!("Failed: {failure_count}"));
    }
  });

  Ok(())
}

/// Find stale branches in a repository
pub fn find_stale_branches<P: AsRef<Path>>(path: P, days: u32) -> Result<()> {
  let path = path.as_ref();
  let repo = Git2Repository::open(path).context(format!("Failed to open git repository at {}", path.display()))?;

  print_info(&format!(
    "Finding branches not updated in the last {} days in {}",
    days,
    format_repo_path(&path.display().to_string())
  ));

  // Calculate the cutoff time
  let now = SystemTime::now();
  let cutoff = now - Duration::from_secs(days as u64 * 24 * 60 * 60);
  let cutoff_secs = cutoff.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

  // Get all branches
  let branches = repo
    .branches(Some(BranchType::Local))
    .context("Failed to get branches")?;

  let mut stale_branches = Vec::new();

  for branch_result in branches {
    let (branch, _) = branch_result.context("Failed to get branch")?;
    let branch_name = branch
      .name()
      .context("Failed to get branch name")?
      .unwrap_or("unknown")
      .to_string();

    // Get the commit that the branch points to
    let commit = branch.get().peel_to_commit().context("Failed to get commit")?;
    let commit_time = commit.time().seconds();

    // Check if the branch is stale
    if commit_time < cutoff_secs {
      let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(commit_time, 0)
        .unwrap()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

      stale_branches.push((branch_name, time_str));
    }
  }

  // Print results
  if stale_branches.is_empty() {
    print_info(&format!(
      "No stale branches found in {}",
      format_repo_path(&path.display().to_string())
    ));
  } else {
    print_warning(&format!(
      "Found {} stale branches in {}:",
      stale_branches.len(),
      format_repo_path(&path.display().to_string())
    ));

    for (name, time) in stale_branches {
      println!(
        "  {} (last commit: {})",
        name,
        crate::utils::output::format_timestamp(&time)
      );
    }
  }

  Ok(())
}

/// Find stale branches in all repositories
pub fn find_stale_branches_all(days: u32) -> Result<()> {
  let config_dirs = crate::config::ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;

  let repos = registry.list();
  if repos.is_empty() {
    print_warning("No repositories in registry.");
    print_info(&format!(
      "Add one with {}",
      crate::utils::output::format_command("twig git add <path>")
    ));
    return Ok(());
  }

  print_info(&format!("Finding stale branches in {} repositories", repos.len()));

  // Create a tokio runtime for parallel execution
  let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

  rt.block_on(async {
    let mut handles = Vec::new();

    // Launch tasks for each repository
    for repo in repos {
      let repo_path = repo.path.clone();
      let repo_name = repo.name.clone();
      let days_value = days;

      let handle = task::spawn(async move {
        let result = find_stale_branches(&repo_path, days_value);
        (repo_name, repo_path, result)
      });

      handles.push(handle);

      // Small delay to avoid overwhelming the system
      time::sleep(Duration::from_millis(100)).await;
    }

    // Wait for all tasks to complete
    let mut success_count = 0;
    let mut failure_count = 0;

    for handle in handles {
      match handle.await {
        Ok((_name, _path, Ok(()))) => {
          success_count += 1;
        }
        Ok((name, path, Err(e))) => {
          print_error(&format!(
            "Error checking stale branches in {} ({}): {}",
            format_repo_name(&name),
            format_repo_path(&path),
            e
          ));
          failure_count += 1;
        }
        Err(e) => {
          print_error(&format!("Task panicked: {e}"));
          failure_count += 1;
        }
      }
    }

    // Print summary
    print_info("Stale branch check complete");
    print_info(&format!("Successful: {success_count}"));

    if failure_count > 0 {
      print_warning(&format!("Failed: {failure_count}"));
    }
  });

  Ok(())
}
