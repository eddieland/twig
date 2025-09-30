//! # Branch Command
//!
//! Derive-based implementation of the branch command for managing branch
//! dependencies and root branches, including adding, removing, and listing
//! branch relationships.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::{RepoState, detect_repository};

use crate::enhanced_errors::ErrorHandler;

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

  /// Reparent all orphaned branches to a specific parent
  #[command(
    long_about = "Add all orphaned branches as children of the specified parent branch.\n\n\
                     This command finds all branches that have no dependencies defined and\n\
                     creates parent-child relationships with the specified parent branch.\n\
                     This is useful for organizing branches that were created without\n\
                     explicit dependencies. Use --dry-run to see which branches would be\n\
                     affected without making actual changes."
  )]
  Reparent(ReparentCommand),

  /// Root branch management
  #[command(long_about = "Manage which branches are treated as root branches.\n\n\
                     Root branches appear at the top level of the tree view\n\
                     and serve as starting points for the dependency tree.")]
  Root(RootCommand),

  /// Clear all dependencies and root branches
  #[command(long_about = "Remove all branch dependencies and root branch configurations.\n\n\
                     This command will completely reset the twig dependency structure,\n\
                     making all branches orphaned. This is useful when you want to\n\
                     start fresh with dependency management or clean up a complex\n\
                     dependency tree. Use --dry-run to preview what would be cleared\n\
                     before making actual changes.")]
  Clear(ClearCommand),
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

/// Reparent all orphaned branches to a specific parent
#[derive(Args)]
pub struct ReparentCommand {
  /// The parent branch name to assign to all orphaned branches
  #[arg(required = true, index = 1)]
  pub parent: String,

  /// Show which branches would be reparented without making changes
  #[arg(long = "dry-run")]
  pub dry_run: bool,

  /// Skip confirmation prompt
  #[arg(short, long)]
  pub force: bool,

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

/// Clear all dependencies and root branches
#[derive(Args)]
pub struct ClearCommand {
  /// Show what would be cleared without making changes
  #[arg(long = "dry-run")]
  pub dry_run: bool,

  /// Skip confirmation prompt
  #[arg(short, long)]
  pub force: bool,

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
        detect_repository().ok_or_else(|| {
          let error = ErrorHandler::handle_repository_error(anyhow::anyhow!("Not in a git repository"));
          error.display_enhanced();
          anyhow::anyhow!(error)
        })?
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
          let enhanced_error = ErrorHandler::handle_branch_error("add dependency", &cmd.child, e);
          enhanced_error.display_enhanced();
          Err(enhanced_error.into())
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
    BranchSubcommands::Reparent(cmd) => handle_reparent_command(cmd),
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
            let enhanced_error = ErrorHandler::handle_branch_error("add root", &cmd.branch, e);
            enhanced_error.display_enhanced();
            Err(enhanced_error.into())
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
    BranchSubcommands::Clear(cmd) => handle_clear_command(cmd),
  }
}

/// Handle the reparent command
///
/// This function finds all orphaned branches (branches with no dependencies)
/// and creates parent-child relationships with the specified parent branch.
fn handle_reparent_command(cmd: ReparentCommand) -> Result<()> {
  use std::io::{self, Write};

  use git2::Repository as Git2Repository;

  // Get the repository path
  let repo_path = if let Some(repo_arg) = cmd.repo {
    crate::utils::resolve_repository_path(Some(&repo_arg))?
  } else {
    detect_repository().context("Not in a git repository")?
  };

  // Open the repository to get branch information
  let repo =
    Git2Repository::open(&repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path).unwrap_or_default();

  // Create the user-defined dependency resolver to identify orphaned branches
  let resolver = crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;

  // Build the branch node tree structure
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;

  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  // Get orphaned branches
  let (_, orphaned_branches) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

  if orphaned_branches.is_empty() {
    print_info("No orphaned branches found. All branches already have dependencies or are root branches.");
    return Ok(());
  }

  // Verify the parent branch exists
  if !branch_nodes.contains_key(&cmd.parent) {
    print_error(&format!("Parent branch '{}' not found", cmd.parent));
    return Err(anyhow::anyhow!("Parent branch does not exist"));
  }

  // Show what would be done
  println!("🔗 Reparenting orphaned branches to '{}':", cmd.parent);
  for branch in &orphaned_branches {
    println!("  • {} -> {}", branch, cmd.parent);
  }

  if cmd.dry_run {
    print_info("Dry run complete. Use --force to apply changes or remove --dry-run to be prompted for confirmation.");
    return Ok(());
  }

  // Confirmation prompt (unless --force is specified)
  if !cmd.force {
    print!(
      "\nProceed with reparenting {} branch(es)? [y/N]: ",
      orphaned_branches.len()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
      print_info("Reparent operation cancelled.");
      return Ok(());
    }
  }

  // Apply the changes
  let mut changes_made = 0;
  let mut failed_changes = Vec::new();

  for branch in &orphaned_branches {
    match repo_state.add_dependency(branch.clone(), cmd.parent.clone()) {
      Ok(()) => {
        changes_made += 1;
      }
      Err(e) => {
        failed_changes.push((branch, e));
      }
    }
  }

  // Save the state if any changes were made
  if changes_made > 0 {
    repo_state.save(&repo_path)?;
    print_success(&format!(
      "Successfully reparented {} branch(es) to '{}'",
      changes_made, cmd.parent
    ));
  }

  // Report any failures
  if !failed_changes.is_empty() {
    print_error(&format!("Failed to reparent {} branch(es):", failed_changes.len()));
    for (branch, error) in failed_changes {
      println!("  • {}: {}", branch, error);
    }
  }

  Ok(())
}

/// Handle the clear command
///
/// This function removes all branch dependencies and root branch
/// configurations, effectively resetting the dependency structure and making
/// all branches orphaned.
fn handle_clear_command(cmd: ClearCommand) -> Result<()> {
  use std::io::{self, Write};

  // Get the repository path
  let repo_path = if let Some(repo_arg) = cmd.repo {
    crate::utils::resolve_repository_path(Some(&repo_arg))?
  } else {
    detect_repository().context("Not in a git repository")?
  };

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path).unwrap_or_default();

  // Count what we're about to clear
  let dependency_count = repo_state.list_dependencies().len();
  let root_count = repo_state.list_roots().len();

  if dependency_count == 0 && root_count == 0 {
    print_info("No dependencies or root branches found. Nothing to clear.");
    return Ok(());
  }

  // Show what would be cleared
  if dependency_count > 0 {
    println!("🗑️  Dependencies to be cleared ({}):", dependency_count);
    for dep in repo_state.list_dependencies() {
      println!("  • {} -> {}", dep.child, dep.parent);
    }
  }

  if root_count > 0 {
    if dependency_count > 0 {
      println!();
    }
    println!("🗑️  Root branches to be cleared ({}):", root_count);
    for root in repo_state.list_roots() {
      let default_marker = if root.is_default { " (default)" } else { "" };
      println!("  • {}{}", root.branch, default_marker);
    }
  }

  if cmd.dry_run {
    print_info("Dry run complete. Use --force to apply changes or remove --dry-run to be prompted for confirmation.");
    return Ok(());
  }

  // Confirmation prompt (unless --force is specified)
  if !cmd.force {
    print_warning("This will completely reset your twig dependency structure!");
    print!("Continue and clear all dependencies and root branches? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
      print_info("Clear operation cancelled.");
      return Ok(());
    }
  }

  // Clear all dependencies and root branches
  let dependencies_cleared = repo_state.dependencies.len();
  let roots_cleared = repo_state.root_branches.len();

  // Clear dependencies by removing all entries
  repo_state.dependencies.clear();

  // Clear root branches by removing all entries
  repo_state.root_branches.clear();

  // Save the cleared state (this will trigger index rebuild on next load)
  repo_state.save(&repo_path)?;

  print_success(&format!(
    "Cleared {} dependencies and {} root branches. All branches are now orphaned.",
    dependencies_cleared, roots_cleared
  ));

  Ok(())
}
