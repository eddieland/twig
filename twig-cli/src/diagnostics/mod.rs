//! # System Diagnostics
//!
//! Provides comprehensive system diagnostics and health checks for twig,
//! including configuration validation, credential checking, and Git repository
//! status.

use std::process::Command;
use std::{env, fs};

use anyhow::Result;

use crate::config::get_config_dirs;
use crate::creds::{check_github_credentials, check_jira_credentials, get_netrc_path};
use crate::git::list_repositories;
use crate::utils::output::{format_repo_path, print_error, print_header, print_success, print_warning};

/// Run comprehensive system diagnostics
pub fn run_diagnostics() -> Result<()> {
  print_header("System Diagnostics");
  println!();

  // Check system information
  check_system_info()?;
  println!();

  // Check configuration directories
  check_config_directories()?;
  println!();

  // Check credentials
  check_credentials()?;
  println!();

  // Check git configuration
  check_git_configuration()?;
  println!();

  // Check tracked repositories
  check_tracked_repositories()?;
  println!();

  // Check dependencies
  check_dependencies()?;
  println!();

  print_success("Diagnostics complete!");

  Ok(())
}

/// Check system information
fn check_system_info() -> Result<()> {
  println!("System Information:");

  // OS information
  let os = env::consts::OS;
  let arch = env::consts::ARCH;
  println!("  Operating System: {os} ({arch})",);

  // Home directory
  if let Some(home) = env::var_os("HOME") {
    println!("  Home Directory: {}", format_repo_path(&home.to_string_lossy()));
  } else {
    println!("  Home Directory: Not found");
  }

  // Current working directory
  match env::current_dir() {
    Ok(cwd) => println!("  Current Directory: {}", format_repo_path(&cwd.display().to_string())),
    Err(e) => println!("  Current Directory: Error - {e}"),
  }

  // Shell information
  if let Ok(shell) = env::var("SHELL") {
    println!("  Shell: {shell}");
  } else {
    println!("  Shell: Not detected");
  }

  Ok(())
}

/// Check configuration directories
fn check_config_directories() -> Result<()> {
  println!("Configuration Directories:");

  let config_dirs = get_config_dirs()?;

  // Config directory
  let config_dir = config_dirs.config_dir();
  if config_dir.exists() {
    println!("  Config: {}", format_repo_path(&config_dir.display().to_string()));
  } else {
    println!(
      "  Config: {} (not created yet)",
      format_repo_path(&config_dir.display().to_string())
    );
  }

  // Data directory
  let data_dir = config_dirs.data_dir();
  if data_dir.exists() {
    println!("  Data: {}", format_repo_path(&data_dir.display().to_string()));

    // Check registry file
    let registry_path = data_dir.join("registry.json");
    if registry_path.exists() {
      println!("  Registry: Found");
    } else {
      println!("  Registry: Not found");
    }
  } else {
    println!(
      "  Data: {} (not created yet)",
      format_repo_path(&data_dir.display().to_string())
    );
  }

  // Cache directory
  if let Some(cache_dir) = config_dirs.cache_dir() {
    if cache_dir.exists() {
      println!("  Cache: {}", format_repo_path(&cache_dir.display().to_string()));
    } else {
      println!(
        "  Cache: {} (not created yet)",
        format_repo_path(&cache_dir.display().to_string())
      );
    }
  } else {
    println!("  Cache: Not configured");
  }

  Ok(())
}

/// Check credentials
fn check_credentials() -> Result<()> {
  println!("Credentials:");

  let netrc_path = get_netrc_path();
  if netrc_path.exists() {
    println!("  .netrc file: {}", format_repo_path(&netrc_path.display().to_string()));

    // Check file permissions
    let metadata = fs::metadata(&netrc_path)?;
    let permissions = metadata.permissions();
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = permissions.mode();
      if mode & 0o077 == 0 {
        println!("  .netrc permissions: Secure (600)");
      } else {
        print_warning(&format!("  .netrc permissions: Insecure ({:o})", mode & 0o777));
      }
    }
    #[cfg(not(unix))]
    {
      println!("  .netrc permissions: Unable to check on this platform");
    }

    // Check specific credentials
    match check_jira_credentials() {
      Ok(true) => println!("  Jira credentials: Found"),
      Ok(false) => println!("  Jira credentials: Not found"),
      Err(e) => print_error(&format!("  Jira credentials: Error - {e}",)),
    }

    match check_github_credentials() {
      Ok(true) => println!("  GitHub credentials: Found"),
      Ok(false) => println!("  GitHub credentials: Not found"),
      Err(e) => print_error(&format!("  GitHub credentials: Error - {e}")),
    }
  } else {
    println!(
      "  .netrc file: {} (not found)",
      format_repo_path(&netrc_path.display().to_string())
    );
  }

  Ok(())
}

/// Check git configuration
fn check_git_configuration() -> Result<()> {
  println!("Git Configuration:");

  // Check if git is available
  match Command::new("git").arg("--version").output() {
    Ok(output) => {
      if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("  Git version: {version}");
      } else {
        print_error("  Git: Command failed");
      }
    }
    Err(e) => {
      print_error(&format!("  Git: Not found or not executable - {e}"));
      return Ok(());
    }
  }

  // Check git configuration
  if let Ok(output) = Command::new("git").args(["config", "--global", "user.name"]).output() {
    if output.status.success() {
      let name = String::from_utf8_lossy(&output.stdout);
      let name = name.trim();
      if !name.is_empty() {
        println!("  User name: {name}");
      } else {
        println!("  User name: Not configured");
      }
    } else {
      println!("  User name: Not configured");
    }
  }

  if let Ok(output) = Command::new("git").args(["config", "--global", "user.email"]).output() {
    if output.status.success() {
      let email = String::from_utf8_lossy(&output.stdout);
      let email = email.trim();
      if !email.is_empty() {
        println!("  User email: {email}");
      } else {
        println!("  User email: Not configured");
      }
    } else {
      println!("  User email: Not configured");
    }
  }

  Ok(())
}

/// Check tracked repositories
fn check_tracked_repositories() -> Result<()> {
  println!("Tracked Repositories:");

  let config_dirs = get_config_dirs()?;
  let registry_path = config_dirs.data_dir().join("registry.json");

  if registry_path.exists() {
    match fs::read_to_string(&registry_path) {
      Ok(content) => {
        if content.trim().is_empty() || content.trim() == "[]" {
          println!("  No repositories tracked");
        } else {
          println!("  Repositories found:");
          // Call list_repositories to show them
          if let Err(e) = list_repositories() {
            print_error(&format!("  Error listing repositories: {e}",));
          }
        }
      }
      Err(e) => print_error(&format!("  Registry read error: {e}",)),
    }
  } else {
    println!("  No repositories tracked (registry not found)");
  }

  Ok(())
}

/// Check dependencies
fn check_dependencies() -> Result<()> {
  println!("Dependencies:");

  // Check curl (for API calls if reqwest fails)
  match Command::new("curl").arg("--version").output() {
    Ok(output) => {
      if output.status.success() {
        let version_line = String::from_utf8_lossy(&output.stdout)
          .lines()
          .next()
          .unwrap_or("curl")
          .to_string();
        println!("  curl: {version_line}");
      } else {
        println!("  curl: Command failed");
      }
    }
    Err(_) => println!("  curl: Not found"),
  }

  // Check ssh (for git operations)
  match Command::new("ssh").arg("-V").output() {
    Ok(output) => {
      if output.status.success() {
        let version = String::from_utf8_lossy(&output.stderr); // ssh -V outputs to stderr
        let version = version.trim();
        println!("  ssh: {version}");
      } else {
        println!("  ssh: Command failed");
      }
    }
    Err(_) => println!("  ssh: Not found"),
  }

  // Check network connectivity to key services
  check_network_connectivity()?;

  Ok(())
}

/// Check network connectivity to key services
fn check_network_connectivity() -> Result<()> {
  println!("Network Connectivity:");

  // This is a simple check - in a real scenario you might want more sophisticated
  // testing
  let services = vec![("GitHub", "github.com"), ("Atlassian", "atlassian.net")];

  for (name, host) in services {
    // Use a simple ping command to test connectivity
    match Command::new("ping").args(["-c", "1", "-W", "3", host]).output() {
      Ok(output) => {
        if output.status.success() {
          println!("  {name}: Reachable");
        } else {
          println!("  {name}: Unreachable",);
        }
      }
      Err(_) => {
        println!("  {name}: Unable to test (ping not available)",);
      }
    }
  }

  Ok(())
}
