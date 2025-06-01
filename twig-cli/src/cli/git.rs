//! # Git Command
//!
//! Derive-based implementation of the git command for managing Git
//! repositories.

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};

/// Command for Git repository management
#[derive(Args)]
pub struct GitArgs {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: GitSubcommands,
}

/// Subcommands for the git command
#[derive(Subcommand)]
pub enum GitSubcommands {
  /// Add a repository to the registry
  #[command(long_about = "Registers a Git repository with twig for tracking and management.\n\n\
                     Once added, the repository can be referenced in other twig commands and\n\
                     included in batch operations. The repository must be a valid Git repository\n\
                     with proper credentials configured if needed for remote operations.")]
  Add(AddCommand),

  /// Remove a repository from the registry
  #[command(
    long_about = "Removes a previously registered Git repository from twig's tracking.\n\n\
                     This only affects twig's registry and does not delete or modify the\n\
                     actual repository files."
  )]
  #[command(alias = "rm")]
  Remove(RemoveCommand),

  /// List all repositories in the registry
  #[command(long_about = "Displays all Git repositories currently registered with twig.\n\n\
                     Shows the repository paths and any additional tracking information\n\
                     to help you manage your repositories.")]
  #[command(alias = "ls")]
  List,

  /// Fetch updates for repositories
  #[command(long_about = "Fetches updates from remote repositories.\n\n\
                     This updates local references to remote branches without modifying your\n\
                     working directory. Requires proper Git credentials to be configured for\n\
                     repositories with private remotes.")]
  Fetch(FetchCommand),

  /// Execute a git command in repositories
  #[command(long_about = "Executes a Git command in one or all registered repositories.\n\n\
                     This powerful feature allows you to run the same Git operation across\n\
                     multiple repositories simultaneously. The command is executed as-is,\n\
                     so ensure it's a valid Git command. Credentials may be required\n\
                     depending on the Git operation being performed.")]
  Exec(ExecCommand),

  /// List stale branches in repositories
  #[command(long_about = "Identifies and lists branches that haven't been updated recently.\n\n\
                     This helps you identify abandoned or forgotten branches that might need\n\
                     attention, cleanup, or merging. The command analyzes local branch information\n\
                     and doesn't require remote credentials unless combined with a fetch operation.")]
  #[command(alias = "stale")]
  StaleBranches(StaleBranchesCommand),
}

/// Add a repository to the registry
#[derive(Args)]
pub struct AddCommand {
  /// Path to the repository
  #[arg(default_value = ".")]
  pub path: String,
}

/// Remove a repository from the registry
#[derive(Args)]
pub struct RemoveCommand {
  /// Path to the repository
  #[arg(default_value = ".")]
  pub path: String,
}

/// Fetch updates for repositories
#[derive(Args)]
pub struct FetchCommand {
  /// Fetch all repositories in the registry
  #[arg(long, short = 'a')]
  pub all: bool,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Execute a git command in repositories
#[derive(Args)]
pub struct ExecCommand {
  /// Execute in all repositories in the registry
  #[arg(long, short = 'a')]
  pub all: bool,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,

  /// Command to execute
  #[arg(required = true, index = 1)]
  pub command: String,
}

/// List stale branches in repositories
#[derive(Args)]
pub struct StaleBranchesCommand {
  /// Number of days to consider a branch stale
  #[arg(long, short = 'd', value_name = "DAYS", default_value = "30")]
  pub days: String,

  /// Check all repositories in the registry
  #[arg(long, short = 'a')]
  pub all: bool,

  /// Path to a specific repository
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Handle the git command
///
/// This function processes the git subcommands and executes the
/// corresponding actions such as adding, removing, listing repositories,
/// fetching updates, executing commands, and finding stale branches.
pub(crate) fn handle_git_command(git: GitArgs) -> Result<()> {
  match git.subcommand {
    GitSubcommands::Add(cmd) => crate::git::add_repository(&cmd.path),
    GitSubcommands::Remove(cmd) => crate::git::remove_repository(&cmd.path),
    GitSubcommands::List => crate::git::list_repositories(),
    GitSubcommands::Fetch(cmd) => {
      if cmd.all {
        crate::git::fetch_all_repositories()
      } else {
        let repo_arg = cmd.repo.as_deref();
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::git::fetch_repository(repo_path, true)
      }
    }
    GitSubcommands::Exec(cmd) => {
      if cmd.all {
        crate::git::execute_all_repositories(&cmd.command)
      } else {
        let repo_arg = cmd.repo.as_deref();
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::git::execute_repository(repo_path, &cmd.command)
      }
    }
    GitSubcommands::StaleBranches(cmd) => {
      let days = cmd
        .days
        .parse::<u32>()
        .map_err(|e| anyhow!("Days must be a positive number: {}", e))?;

      if cmd.all {
        crate::git::find_stale_branches_all(days)
      } else {
        let repo_arg = cmd.repo.as_deref();
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::git::find_stale_branches(repo_path, days)
      }
    }
  }
}
