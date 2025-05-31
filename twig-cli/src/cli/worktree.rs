use anyhow::Result;
use clap::{Arg, Command};

/// Build the worktree subcommand
pub fn build_command() -> Command {
  Command::new("worktree")
    .about("Worktree management")
    .long_about(
      "Manage Git worktrees for efficient multi-branch development.\n\n\
            Worktrees allow you to check out multiple branches simultaneously in separate\n\
            directories, all connected to the same repository. This enables working on\n\
            different features or fixes concurrently without stashing or committing\n\
            incomplete work.",
    )
    .arg_required_else_help(true)
    .alias("wt")
    .subcommand(
      Command::new("create")
        .about("Create a new worktree for a branch")
        .long_about(
          "Creates a new Git worktree for a specific branch.\n\n\
                    This allows you to work on multiple branches simultaneously without switching\n\
                    branches in your main repository. If the branch doesn't exist, it will be\n\
                    created. The worktree will be created in a directory named after the branch\n\
                    within the repository's parent directory.",
        )
        .alias("new")
        .arg(Arg::new("branch").help("Branch name").required(true))
        .arg(
          Arg::new("repo")
            .long("repo")
            .short('r')
            .help("Path to a specific repository")
            .value_name("PATH"),
        ),
    )
    .subcommand(
      Command::new("list")
        .about("List all worktrees for a repository")
        .long_about(
          "Lists all worktrees associated with a Git repository.\n\n\
                    Shows the path, branch name, and commit information for each worktree\n\
                    to help you track your active development environments.",
        )
        .alias("ls")
        .arg(
          Arg::new("repo")
            .long("repo")
            .short('r')
            .help("Path to a specific repository")
            .value_name("PATH"),
        ),
    )
    .subcommand(
      Command::new("clean")
        .about("Clean up stale worktrees")
        .long_about(
          "Removes worktrees that are no longer needed or have been abandoned.\n\n\
                    This helps keep your workspace clean and organized. The command checks for\n\
                    worktrees with branches that have been merged or deleted and offers to\n\
                    remove them. This operation only removes the worktree directories and\n\
                    doesn't affect the main repository.",
        )
        .arg(
          Arg::new("repo")
            .long("repo")
            .short('r')
            .help("Path to a specific repository")
            .value_name("PATH"),
        ),
    )
}

/// Handle worktree subcommands
pub fn handle_commands(worktree_matches: &clap::ArgMatches) -> Result<()> {
  match worktree_matches.subcommand() {
    Some(("create", create_matches)) => {
      let branch = create_matches.get_one::<String>("branch").unwrap();
      let repo_arg = create_matches.get_one::<String>("repo").map(|s| s.as_str());
      let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
      crate::repo_state::create_worktree(repo_path, branch)?;
      Ok(())
    }
    Some(("list", list_matches)) => {
      let repo_arg = list_matches.get_one::<String>("repo").map(|s| s.as_str());
      let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
      crate::repo_state::list_worktrees(repo_path)
    }
    Some(("clean", clean_matches)) => {
      let repo_arg = clean_matches.get_one::<String>("repo").map(|s| s.as_str());
      let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
      crate::repo_state::clean_worktrees(repo_path)
    }
    _ => {
      use crate::utils::output::print_warning;
      print_warning("Unknown worktree command.");
      Ok(())
    }
  }
}
