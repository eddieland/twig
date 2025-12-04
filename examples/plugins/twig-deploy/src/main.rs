//! # Twig Deploy Plugin
//!
//! Example plugin demonstrating how to create a twig plugin in Rust using
//! twig-core.

use std::env;

use anyhow::Result;
use clap::{Parser, Subcommand};
use twig_core::{detect_repository, plugin, print_error, print_info, print_success};

#[derive(Parser)]
#[command(name = "twig-deploy")]
#[command(about = "Deploy applications using twig context")]
#[command(version = "1.0.0")]
struct Cli {
  #[command(subcommand)]
  command: Commands,

  /// Increase verbosity
  #[arg(short, long, action = clap::ArgAction::Count)]
  verbose: u8,

  /// Control colored output
  #[arg(long, value_enum, default_value = "auto")]
  color: ColorMode,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum ColorMode {
  Auto,
  Always,
  Never,
}

#[derive(Subcommand)]
enum Commands {
  /// Deploy to staging environment
  Staging {
    /// Force deployment even if checks fail
    #[arg(short, long)]
    force: bool,
  },
  /// Deploy to production environment
  Production {
    /// Force deployment even if checks fail
    #[arg(short, long)]
    force: bool,
    /// Require confirmation for production deployment
    #[arg(long, default_value = "true")]
    confirm: bool,
  },
  /// Show deployment status
  Status,
}

fn main() -> Result<()> {
  let cli = Cli::parse();

  // Get verbosity from twig environment variable, fall back to CLI args
  let verbosity = env::var("TWIG_VERBOSITY")
    .ok()
    .and_then(|v| v.parse::<u8>().ok())
    .unwrap_or(cli.verbose);

  // Get twig context
  let config_dir = plugin::plugin_config_dir("deploy")?;
  let data_dir = plugin::plugin_data_dir("deploy")?;

  if verbosity > 0 {
    print_info(&format!("Plugin config dir: {}", config_dir.display()));
    print_info(&format!("Plugin data dir: {}", data_dir.display()));
  }

  if verbosity > 1 {
    print_info(&format!("Verbosity level: {}", verbosity));
    if let Ok(twig_version) = env::var("TWIG_VERSION") {
      print_info(&format!("Twig version: {}", twig_version));
    }
  }

  // Check if we're in a git repository
  if !plugin::in_git_repository() {
    print_error("Not in a git repository. Please run this command from within a git repository.");
    std::process::exit(1);
  }

  // Get current repository and branch
  if let Some(repo) = detect_repository() {
    print_info(&format!("Repository: {}", repo.display()));
  }

  if let Some(branch) = plugin::current_branch()? {
    print_info(&format!("Current branch: {}", branch));
  }

  match cli.command {
    Commands::Staging { force } => {
      print_info("Deploying to staging environment...");
      if force {
        print_info("Force deployment enabled");
      }

      // Simulate deployment
      std::thread::sleep(std::time::Duration::from_millis(500));
      print_success("Successfully deployed to staging!");
    }
    Commands::Production { force, confirm } => {
      if confirm && !force {
        print_info("Production deployment requires confirmation.");
        print_info("Use --force to skip confirmation or implement interactive confirmation.");
      }

      print_info("Deploying to production environment...");
      if force {
        print_info("Force deployment enabled");
      }

      // Simulate deployment
      std::thread::sleep(std::time::Duration::from_millis(1000));
      print_success("Successfully deployed to production!");
    }
    Commands::Status => {
      print_info("Checking deployment status...");

      // Simulate status check
      std::thread::sleep(std::time::Duration::from_millis(300));
      print_success("All deployments are healthy!");
    }
  }

  Ok(())
}
