//! # Node.js Integration Commands
//!
//! Commands for detecting and integrating with Node.js projects.

use anyhow::Result;
use clap::{Args, Subcommand};
use twig_core::{nodejs::NodeJsTooling, output::print_info};

/// Node.js integration commands
#[derive(Args)]
pub struct NodeJsArgs {
  #[command(subcommand)]
  pub command: NodeJsCommands,
}

/// Node.js subcommands
#[derive(Subcommand)]
pub enum NodeJsCommands {
  /// Detect Node.js projects in current or specified directory
  #[command(long_about = "Detect Node.js projects and show project information.\n\n\
            This command checks if the current or specified directory contains a\n\
            Node.js project by looking for package.json files. If found, it shows\n\
            project details including name, version, scripts, and detected package manager.")]
  Detect {
    /// Path to check for Node.js project (defaults to current directory)
    #[arg(short, long)]
    path: Option<std::path::PathBuf>,
  },

  /// Show available npm/yarn/pnpm scripts from package.json
  #[command(long_about = "Display all available scripts from package.json.\n\n\
            This command parses the package.json file and displays all defined scripts\n\
            along with their commands. Useful for discovering available build, test,\n\
            and development scripts in a Node.js project.")]
  Scripts {
    /// Path to Node.js project (defaults to current directory)
    #[arg(short, long)]
    path: Option<std::path::PathBuf>,
  },

  /// Enhance .gitignore with Node.js patterns
  #[command(long_about = "Add Node.js-specific patterns to .gitignore file.\n\n\
            This command adds comprehensive Node.js ignore patterns including node_modules,\n\
            build artifacts, environment files, cache directories, and common editor files.\n\
            Only works on directories containing a package.json file.")]
  Gitignore {
    /// Path to Node.js project (defaults to current directory)
    #[arg(short, long)]
    path: Option<std::path::PathBuf>,
    
    /// Show what would be added without actually modifying .gitignore
    #[arg(long)]
    dry_run: bool,
  },
}

pub fn handle_nodejs_command(args: NodeJsArgs) -> Result<()> {
  match args.command {
    NodeJsCommands::Detect { path } => handle_detect_command(path),
    NodeJsCommands::Scripts { path } => handle_scripts_command(path),
    NodeJsCommands::Gitignore { path, dry_run } => handle_gitignore_command(path, dry_run),
  }
}

fn handle_detect_command(path: Option<std::path::PathBuf>) -> Result<()> {
  let project_path = path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
  
  if !NodeJsTooling::detect_project(&project_path) {
    print_info(&format!("No Node.js project detected at: {}", project_path.display()));
    return Ok(());
  }

  print_info(&format!("Node.js project detected at: {}", project_path.display()));
  
  // Parse and display package.json information
  match NodeJsTooling::parse_package_json(&project_path) {
    Ok(package_json) => {
      if let Some(name) = &package_json.name {
        print_info(&format!("  Name: {}", name));
      }
      if let Some(version) = &package_json.version {
        print_info(&format!("  Version: {}", version));
      }
      if let Some(description) = &package_json.description {
        print_info(&format!("  Description: {}", description));
      }
      
      // Show package manager
      if let Some(package_manager) = NodeJsTooling::detect_package_manager(&project_path) {
        let pm_name = match package_manager {
          twig_core::nodejs::PackageManager::Npm => "npm",
          twig_core::nodejs::PackageManager::Yarn => "yarn", 
          twig_core::nodejs::PackageManager::Pnpm => "pnpm",
        };
        print_info(&format!("  Package Manager: {}", pm_name));
      }
      
      // Show script count
      if let Some(scripts) = &package_json.scripts {
        print_info(&format!("  Scripts: {} available", scripts.len()));
      }
    }
    Err(e) => {
      print_info(&format!("  Error reading package.json: {}", e));
    }
  }
  
  Ok(())
}

fn handle_scripts_command(path: Option<std::path::PathBuf>) -> Result<()> {
  let project_path = path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
  
  if !NodeJsTooling::detect_project(&project_path) {
    print_info("No Node.js project detected in current directory.");
    return Ok(());
  }

  match NodeJsTooling::get_available_scripts(&project_path) {
    Ok(scripts) => {
      if scripts.is_empty() {
        print_info("No scripts defined in package.json");
      } else {
        print_info("Available scripts:");
        for (name, command) in scripts {
          print_info(&format!("  {}: {}", name, command));
        }
      }
    }
    Err(e) => {
      print_info(&format!("Error reading scripts: {}", e));
    }
  }
  
  Ok(())
}

fn handle_gitignore_command(path: Option<std::path::PathBuf>, dry_run: bool) -> Result<()> {
  let project_path = path.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
  
  if !NodeJsTooling::detect_project(&project_path) {
    print_info("No Node.js project detected in current directory.");
    return Ok(());
  }

  if dry_run {
    print_info("Node.js gitignore patterns that would be added:");
    for pattern in NodeJsTooling::get_nodejs_gitignore_patterns() {
      if !pattern.starts_with('#') && !pattern.is_empty() {
        print_info(&format!("  {}", pattern));
      }
    }
  } else {
    match NodeJsTooling::enhance_gitignore(&project_path) {
      Ok(true) => {
        print_info("Successfully added Node.js patterns to .gitignore");
      }
      Ok(false) => {
        print_info("Node.js patterns already present in .gitignore");
      }
      Err(e) => {
        print_info(&format!("Error enhancing .gitignore: {}", e));
      }
    }
  }
  
  Ok(())
}