//! # Branch Command
//!
//! Derive-based implementation of the branch command for managing branch
//! dependencies and root branches, including adding, removing, and listing
//! branch relationships.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::utils::output::{print_error, print_info, print_success, print_warning};

/// Command for branch dependency and root management
#[derive(Args)]
pub struct BranchArgs {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: BranchSubcommands,
}

/// Subcommands for the branch command
#[derive(Subcommand)]
pub enum BranchSubcommands {
  /// Add a dependency between branches
  #[command(long_about = "Create a parent-child relationship between two branches.\n\n\
                     This allows you to define custom dependencies that will be used\n\
                     in tree rendering. The child branch will appear as a child of\n\
                     the parent branch in the tree view.")]
  Depend(DependCommand),

  /// Remove a dependency between branches
  #[command(long_about = "Remove a previously defined parent-child relationship.\n\n\
                     This removes the custom dependency between two branches,\n\
                     allowing the tree view to fall back to Git's automatic\n\
                     detection for these branches.")]
  #[command(alias = "rm-dep")]
  RemoveDep(RemoveDepCommand),

  /// Root branch management
  #[command(long_about = "Manage which branches are treated as root branches.\n\n\
                     Root branches appear at the top level of the tree view\n\
                     and serve as starting points for the dependency tree.")]
  Root(RootCommand),
}

/// Add a dependency between branches
#[derive(Args)]
pub struct DependCommand {
  /// The child branch name
  #[arg(required = true, index = 1)]
  pub child: String,

  /// The parent branch name
  #[arg(required = true, index = 2)]
  pub parent: String,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Remove a dependency between branches
#[derive(Args)]
pub struct RemoveDepCommand {
  /// The child branch name
  #[arg(required = true, index = 1)]
  pub child: String,

  /// The parent branch name
  #[arg(required = true, index = 2)]
  pub parent: String,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Root branch management
#[derive(Args)]
pub struct RootCommand {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: RootSubcommands,
}

/// Subcommands for the root command
#[derive(Subcommand)]
pub enum RootSubcommands {
  /// Add a root branch
  #[command(long_about = "Mark a branch as a root branch.\n\n\
                         Root branches appear at the top level of the tree view.\n\
                         You can optionally set a root branch as the default,\n\
                         which will be used when no specific root is specified.")]
  Add(RootAddCommand),

  /// List all root branches
  #[command(long_about = "Display all branches currently marked as root branches.\n\n\
                         Shows which branch (if any) is set as the default root.")]
  #[command(alias = "ls")]
  List(RootListCommand),

  /// Remove a root branch
  #[command(long_about = "Remove a branch from the list of root branches.\n\n\
                         This will remove the branch from the root branch list.\n\
                         If it was the default root, the default will be cleared.")]
  #[command(alias = "rm")]
  Remove(RootRemoveCommand),
}

/// Add a root branch
#[derive(Args)]
pub struct RootAddCommand {
  /// The branch name to add as root
  #[arg(required = true, index = 1)]
  pub branch: String,

  /// Set this as the default root branch
  #[arg(long)]
  pub default: bool,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Remove a root branch
#[derive(Args)]
pub struct RootRemoveCommand {
  /// The branch name to remove from roots
  #[arg(required = true, index = 1)]
  pub branch: String,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// List all root branches
#[derive(Args)]
pub struct RootListCommand {
  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Handle the branch command
///
/// This function processes the branch command and its subcommands,
/// including adding and removing dependencies, and managing root branches.
pub(crate) fn handle_branch_command(branch: BranchArgs) -> Result<()> {
  match branch.subcommand {
    BranchSubcommands::Depend(cmd) => {
      // Get the repository path
      let repo_path = if let Some(repo_arg) = cmd.repo {
        crate::utils::resolve_repository_path(Some(&repo_arg))?
      } else {
        detect_current_repository().context("Not in a git repository")?
      };

      // Load repository state
      let mut repo_state = RepoState::load(&repo_path)?;

      // Add the dependency
      match repo_state.add_dependency(cmd.child.clone(), cmd.parent.clone()) {
        Ok(()) => {
          // Save the state
          repo_state.save(&repo_path)?;
          print_success(&format!("Added dependency: {} -> {}", cmd.child, cmd.parent));
          Ok(())
        }
        Err(e) => {
          print_error(&format!("Failed to add dependency: {e}"));
          Err(e)
        }
      }
    }
    BranchSubcommands::RemoveDep(cmd) => {
      // Get the repository path
      let repo_path = if let Some(repo_arg) = cmd.repo {
        crate::utils::resolve_repository_path(Some(&repo_arg))?
      } else {
        detect_current_repository().context("Not in a git repository")?
      };

      // Load repository state
      let mut repo_state = RepoState::load(&repo_path)?;

      // Remove the dependency
      if repo_state.remove_dependency(&cmd.child, &cmd.parent) {
        // Save the state
        repo_state.save(&repo_path)?;
        print_success(&format!("Removed dependency: {} -> {}", cmd.child, cmd.parent));
      } else {
        print_warning(&format!("Dependency {} -> {} not found", cmd.child, cmd.parent));
      }

      Ok(())
    }
    BranchSubcommands::Root(root_cmd) => match root_cmd.subcommand {
      RootSubcommands::Add(cmd) => {
        // Get the repository path
        let repo_path = if let Some(repo_arg) = cmd.repo {
          crate::utils::resolve_repository_path(Some(&repo_arg))?
        } else {
          detect_current_repository().context("Not in a git repository")?
        };

        // Load repository state
        let mut repo_state = RepoState::load(&repo_path)?;

        // Add the root branch
        match repo_state.add_root(cmd.branch.clone(), cmd.default) {
          Ok(()) => {
            // Save the state
            repo_state.save(&repo_path)?;
            if cmd.default {
              print_success(&format!("Added {} as default root branch", cmd.branch));
            } else {
              print_success(&format!("Added {} as root branch", cmd.branch));
            }
            Ok(())
          }
          Err(e) => {
            print_error(&format!("Failed to add root branch: {e}"));
            Err(e)
          }
        }
      }
      RootSubcommands::List(cmd) => {
        // Get the repository path
        let repo_path = if let Some(repo_arg) = cmd.repo {
          crate::utils::resolve_repository_path(Some(&repo_arg))?
        } else {
          detect_current_repository().context("Not in a git repository")?
        };

        // Load repository state
        let repo_state = RepoState::load(&repo_path)?;

        // List all root branches
        let roots = repo_state.list_roots();
        let default_root = repo_state.get_default_root();

        if roots.is_empty() {
          print_info("No root branches defined");
        } else {
          print_info("Root branches:");
          for root in roots {
            if Some(root.branch.as_str()) == default_root {
              print_info(&format!("  {} (default)", root.branch));
            } else {
              print_info(&format!("  {}", root.branch));
            }
          }
        }

        Ok(())
      }
      RootSubcommands::Remove(cmd) => {
        // Get the repository path
        let repo_path = if let Some(repo_arg) = cmd.repo {
          crate::utils::resolve_repository_path(Some(&repo_arg))?
        } else {
          detect_current_repository().context("Not in a git repository")?
        };

        // Load repository state
        let mut repo_state = RepoState::load(&repo_path)?;

        // Remove the root branch
        if repo_state.remove_root(&cmd.branch) {
          // Save the state
          repo_state.save(&repo_path)?;
          print_success(&format!("Removed {} from root branches", cmd.branch));
        } else {
          print_warning(&format!("Root branch {} not found", cmd.branch));
        }

        Ok(())
      }
    },
  }
}
