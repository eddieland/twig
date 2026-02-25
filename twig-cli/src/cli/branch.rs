//! # Branch Command
//!
//! Derive-based implementation of the branch command for managing branch
//! dependencies and root branches, including adding, removing, and listing
//! branch relationships.

use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::{RepoState, detect_repository};

use crate::complete::branch_completer;

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
  #[command(long_about = "Remove a previously defined parent-child relationship.")]
  #[command(alias = "rm-dep")]
  RemoveDep(RemoveDepCommand),

  /// Show the parent branch(es) of the current or specified branch
  #[command(
    long_about = "Display the parent branch(es) of the current branch or a specified branch.\n\n\
                     Shows all direct parent dependencies that have been defined\n\
                     for the branch. If the branch has no defined parents, shows\n\
                     the Git upstream branch if available."
  )]
  Parent(ParentCommand),

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
  #[arg(required = true, index = 1, add = branch_completer())]
  pub child: String,

  /// The parent branch name
  #[arg(required = true, index = 2, add = branch_completer())]
  pub parent: String,

  /// Remove all other parent dependencies for the child before adding this one
  #[arg(
    long,
    help = "Remove every other parent dependency for the child before adding this one"
  )]
  pub exclusive: bool,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Remove a dependency between branches
#[derive(Args)]
pub struct RemoveDepCommand {
  /// The child branch name
  #[arg(required = true, index = 1, add = branch_completer())]
  pub child: String,

  /// The parent branch name
  #[arg(required = true, index = 2, add = branch_completer())]
  pub parent: String,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Show parent branch(es) of a branch
#[derive(Args)]
pub struct ParentCommand {
  /// The branch name (defaults to current branch)
  #[arg(index = 1, add = branch_completer())]
  pub branch: Option<String>,

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
  #[arg(required = true, index = 1, add = branch_completer())]
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
  #[arg(required = true, index = 1, add = branch_completer())]
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
    BranchSubcommands::Parent(cmd) => {
      // Get the repository path
      let repo_path = if let Some(repo_arg) = cmd.repo {
        crate::utils::resolve_repository_path(Some(&repo_arg))?
      } else {
        detect_repository().context("Not in a git repository")?
      };

      // Get the branch name (current or specified)
      let branch_name = if let Some(branch) = cmd.branch {
        resolve_branch_alias(&repo_path, &branch)?
      } else {
        current_branch_name(&repo_path)?
      };

      // Load repository state
      let repo_state = RepoState::load(&repo_path)?;

      // Get parent dependencies for the branch using indexed O(1) lookup
      let parents: Vec<_> = repo_state
        .get_dependency_parents(&branch_name)
        .into_iter()
        .map(|s| s.to_string())
        .collect();

      if parents.is_empty() {
        // Check for Git upstream branch
        let repo = git2::Repository::open(&repo_path).context("Failed to open repository")?;

        if let Ok(branch) = repo.find_branch(&branch_name, git2::BranchType::Local) {
          if let Ok(upstream) = branch.upstream() {
            if let Some(upstream_name) = upstream.name()? {
              print_info(&format!(
                "No twig parent defined for '{}', but Git upstream is: {}",
                branch_name, upstream_name
              ));
            } else {
              print_info(&format!("No parent branches defined for '{}'", branch_name));
            }
          } else {
            print_info(&format!("No parent branches defined for '{}'", branch_name));
          }
        } else {
          print_info(&format!("No parent branches defined for '{}'", branch_name));
        }
      } else if parents.len() == 1 {
        print_success(&format!("Parent branch of '{}': {}", branch_name, parents[0]));
      } else {
        print_success(&format!("Parent branches of '{}':", branch_name));
        for parent in parents {
          print_info(&format!("  {}", parent));
        }
      }

      Ok(())
    }
    BranchSubcommands::Depend(cmd) => {
      // Get the repository path
      let repo_path = if let Some(repo_arg) = cmd.repo {
        crate::utils::resolve_repository_path(Some(&repo_arg))?
      } else {
        detect_repository().context("Not in a git repository")?
      };

      // Load repository state
      let mut repo_state = RepoState::load(&repo_path)?;
      let child = resolve_branch_alias(&repo_path, &cmd.child)?;
      let parent = resolve_branch_alias(&repo_path, &cmd.parent)?;

      if cmd.exclusive {
        let removed_parents = repo_state.remove_child_dependencies(&child);
        if removed_parents.is_empty() {
          print_info(&format!(
            "No existing parent dependencies to remove for '{}'.",
            child
          ));
        } else {
          print_info(&format!(
            "Removed {} parent dependency(ies) for '{}': {}",
            removed_parents.len(),
            child,
            removed_parents.join(", ")
          ));
        }
      }

      // Add the dependency
      match repo_state.add_dependency(child.clone(), parent.clone()) {
        Ok(()) => {
          // Save the state
          repo_state.save(&repo_path)?;
          print_success(&format!("Added dependency: {child} -> {parent}"));
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
        detect_repository().context("Not in a git repository")?
      };

      // Load repository state
      let mut repo_state = RepoState::load(&repo_path)?;
      let child = resolve_branch_alias(&repo_path, &cmd.child)?;
      let parent = resolve_branch_alias(&repo_path, &cmd.parent)?;

      // Remove the dependency
      if repo_state.remove_dependency(&child, &parent) {
        // Save the state
        repo_state.save(&repo_path)?;
        print_success(&format!("Removed dependency: {child} -> {parent}"));
      } else {
        print_warning(&format!("Dependency {child} -> {parent} not found"));
      }

      Ok(())
    }
    BranchSubcommands::Root(root_cmd) => match root_cmd.subcommand {
      RootSubcommands::Add(cmd) => {
        // Get the repository path
        let repo_path = if let Some(repo_arg) = cmd.repo {
          crate::utils::resolve_repository_path(Some(&repo_arg))?
        } else {
          detect_repository().context("Not in a git repository")?
        };

        // Load repository state
        let mut repo_state = RepoState::load(&repo_path)?;
        let branch = resolve_branch_alias(&repo_path, &cmd.branch)?;

        // Add the root branch
        match repo_state.add_root(branch.clone(), cmd.default) {
          Ok(()) => {
            // Save the state
            repo_state.save(&repo_path)?;
            if cmd.default {
              print_success(&format!("Added {branch} as default root branch"));
            } else {
              print_success(&format!("Added {branch} as root branch"));
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
          detect_repository().context("Not in a git repository")?
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
          detect_repository().context("Not in a git repository")?
        };

        // Load repository state
        let mut repo_state = RepoState::load(&repo_path)?;
        let branch = resolve_branch_alias(&repo_path, &cmd.branch)?;

        // Remove the root branch
        if repo_state.remove_root(&branch) {
          // Save the state
          repo_state.save(&repo_path)?;
          print_success(&format!("Removed {branch} from root branches"));
        } else {
          print_warning(&format!("Root branch {branch} not found"));
        }

        Ok(())
      }
    },
  }
}

/// Resolve branch aliases like ".".
///
/// This function checks if the provided branch name is an alias (like ".")
/// and resolves it to the actual branch name. If the branch name is not an
/// alias, it is returned as-is.
///
/// Arguments:
/// - `repo_path`: Path to the Git repository.
/// - `branch`: The input branch name or alias to resolve.
///
/// Returns:
/// - `Ok(String)`: The resolved branch name.
/// - `Err(anyhow::Error)`: An error if the alias cannot be resolved.
fn resolve_branch_alias(repo_path: &Path, branch: &str) -> Result<String> {
  if branch == "." {
    current_branch_name(repo_path)
  } else {
    Ok(branch.to_string())
  }
}

/// Get the current branch name of the repository at the given path.
///
/// This function will open the Git repository located at `repo_path` and
/// return the name of the currently checked-out branch. If the repository
/// is in a detached HEAD state or if any error occurs, an appropriate error
/// will be returned.
///
/// Arguments:
/// - `repo_path`: Path to the Git repository.
///
/// Returns:
/// - `Ok(String)`: The name of the current branch.
/// - `Err(anyhow::Error)`: An error if the repository cannot be opened or
fn current_branch_name(repo_path: &Path) -> Result<String> {
  let repo = git2::Repository::open(repo_path).context("Failed to open repository")?;
  let head = repo.head().context("Failed to get HEAD")?;
  let branch_ref = head.shorthand().context("Failed to get branch name")?;
  Ok(branch_ref.to_string())
}
