//! # Branch Command
//!
//! CLI commands for managing branch dependencies and root branches,
//! including adding, removing, and listing branch relationships.

use anyhow::{Context, Result};
use clap::{Arg, Command};

use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::utils::output::{print_error, print_info, print_success, print_warning};

/// Build the branch command
pub fn build_command() -> Command {
  Command::new("branch")
    .about("Branch dependency and root management")
    .long_about(
      "Manage custom branch dependencies and root branches.\n\n\
            This command group allows you to define custom parent-child relationships\n\
            between branches beyond Git's automatic detection. You can also manage\n\
            which branches should be treated as root branches in the tree view.",
    )
    .alias("br")
    .arg_required_else_help(true)
    .subcommand(
      Command::new("depend")
        .about("Add a dependency between branches")
        .long_about(
          "Create a parent-child relationship between two branches.\n\n\
                    This allows you to define custom dependencies that will be used\n\
                    in tree rendering. The child branch will appear as a child of\n\
                    the parent branch in the tree view.",
        )
        .arg(Arg::new("child").help("The child branch name").required(true).index(1))
        .arg(
          Arg::new("parent")
            .help("The parent branch name")
            .required(true)
            .index(2),
        )
        .arg(
          Arg::new("repo")
            .long("repo")
            .short('r')
            .help("Path to a specific repository")
            .value_name("PATH"),
        ),
    )
    .subcommand(
      Command::new("remove-dep")
        .about("Remove a dependency between branches")
        .long_about(
          "Remove a previously defined parent-child relationship.\n\n\
                    This removes the custom dependency between two branches,\n\
                    allowing the tree view to fall back to Git's automatic\n\
                    detection for these branches.",
        )
        .alias("rm-dep")
        .arg(Arg::new("child").help("The child branch name").required(true).index(1))
        .arg(
          Arg::new("parent")
            .help("The parent branch name")
            .required(true)
            .index(2),
        )
        .arg(
          Arg::new("repo")
            .long("repo")
            .short('r')
            .help("Path to a specific repository")
            .value_name("PATH"),
        ),
    )
    .subcommand(
      Command::new("root")
        .about("Root branch management")
        .long_about(
          "Manage which branches are treated as root branches.\n\n\
                    Root branches appear at the top level of the tree view\n\
                    and serve as starting points for the dependency tree.",
        )
        .arg_required_else_help(true)
        .subcommand(
          Command::new("add")
            .about("Add a root branch")
            .long_about(
              "Mark a branch as a root branch.\n\n\
                        Root branches appear at the top level of the tree view.\n\
                        You can optionally set a root branch as the default,\n\
                        which will be used when no specific root is specified.",
            )
            .arg(
              Arg::new("branch")
                .help("The branch name to add as root")
                .required(true)
                .index(1),
            )
            .arg(
              Arg::new("default")
                .long("default")
                .help("Set this as the default root branch")
                .action(clap::ArgAction::SetTrue),
            )
            .arg(
              Arg::new("repo")
                .long("repo")
                .short('r')
                .help("Path to a specific repository")
                .value_name("PATH"),
            ),
        )
        .subcommand(
          Command::new("remove")
            .about("Remove a root branch")
            .long_about(
              "Remove a branch from the list of root branches.\n\n\
                        This will remove the branch from the root branch list.\n\
                        If it was the default root, the default will be cleared.",
            )
            .alias("rm")
            .arg(
              Arg::new("branch")
                .help("The branch name to remove from roots")
                .required(true)
                .index(1),
            )
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
            .about("List all root branches")
            .long_about(
              "Display all branches currently marked as root branches.\n\n\
                        Shows which branch (if any) is set as the default root.",
            )
            .alias("ls")
            .arg(
              Arg::new("repo")
                .long("repo")
                .short('r')
                .help("Path to a specific repository")
                .value_name("PATH"),
            ),
        ),
    )
}

/// Handle branch commands
pub fn handle_commands(branch_matches: &clap::ArgMatches) -> Result<()> {
  match branch_matches.subcommand() {
    Some(("depend", depend_matches)) => handle_depend_command(depend_matches),
    Some(("remove-dep", remove_dep_matches)) => handle_remove_dep_command(remove_dep_matches),
    Some(("root", root_matches)) => handle_root_commands(root_matches),
    _ => {
      print_error("Unknown branch command");
      Ok(())
    }
  }
}

/// Handle the depend command
fn handle_depend_command(depend_matches: &clap::ArgMatches) -> Result<()> {
  let child = depend_matches.get_one::<String>("child").unwrap();
  let parent = depend_matches.get_one::<String>("parent").unwrap();

  // Get the repository path
  let repo_path = if let Some(repo_arg) = depend_matches.get_one::<String>("repo") {
    crate::utils::resolve_repository_path(Some(repo_arg.as_str()))?
  } else {
    detect_current_repository().context("Not in a git repository")?
  };

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path)?;

  // Add the dependency
  match repo_state.add_dependency(child.clone(), parent.clone()) {
    Ok(()) => {
      // Save the state
      repo_state.save(&repo_path)?;
      print_success(&format!("Added dependency: {child} -> {parent}"));
      Ok(())
    }
    Err(e) => {
      print_error(&format!("Failed to add dependency: {e}",));
      Err(e)
    }
  }
}

/// Handle the remove-dep command
fn handle_remove_dep_command(remove_dep_matches: &clap::ArgMatches) -> Result<()> {
  let child = remove_dep_matches.get_one::<String>("child").unwrap();
  let parent = remove_dep_matches.get_one::<String>("parent").unwrap();

  // Get the repository path
  let repo_path = if let Some(repo_arg) = remove_dep_matches.get_one::<String>("repo") {
    crate::utils::resolve_repository_path(Some(repo_arg.as_str()))?
  } else {
    detect_current_repository().context("Not in a git repository")?
  };

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path)?;

  // Remove the dependency
  if repo_state.remove_dependency(child, parent) {
    // Save the state
    repo_state.save(&repo_path)?;
    print_success(&format!("Removed dependency: {child} -> {parent}",));
  } else {
    print_warning(&format!("Dependency {child} -> {parent} not found",));
  }

  Ok(())
}

/// Handle root subcommands
fn handle_root_commands(root_matches: &clap::ArgMatches) -> Result<()> {
  match root_matches.subcommand() {
    Some(("add", add_matches)) => handle_root_add_command(add_matches),
    Some(("remove", remove_matches)) => handle_root_remove_command(remove_matches),
    Some(("list", list_matches)) => handle_root_list_command(list_matches),
    _ => {
      print_error("Unknown root command");
      Ok(())
    }
  }
}

/// Handle the root add command
fn handle_root_add_command(add_matches: &clap::ArgMatches) -> Result<()> {
  let branch = add_matches.get_one::<String>("branch").unwrap();
  let is_default = add_matches.get_flag("default");

  // Get the repository path
  let repo_path = if let Some(repo_arg) = add_matches.get_one::<String>("repo") {
    crate::utils::resolve_repository_path(Some(repo_arg.as_str()))?
  } else {
    detect_current_repository().context("Not in a git repository")?
  };

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path)?;

  // Add the root branch
  match repo_state.add_root(branch.clone(), is_default) {
    Ok(()) => {
      // Save the state
      repo_state.save(&repo_path)?;
      if is_default {
        print_success(&format!("Added {branch} as default root branch",));
      } else {
        print_success(&format!("Added {branch} as root branch",));
      }
      Ok(())
    }
    Err(e) => {
      print_error(&format!("Failed to add root branch: {e}",));
      Err(e)
    }
  }
}

/// Handle the root remove command
fn handle_root_remove_command(remove_matches: &clap::ArgMatches) -> Result<()> {
  let branch = remove_matches.get_one::<String>("branch").unwrap();

  // Get the repository path
  let repo_path = if let Some(repo_arg) = remove_matches.get_one::<String>("repo") {
    crate::utils::resolve_repository_path(Some(repo_arg.as_str()))?
  } else {
    detect_current_repository().context("Not in a git repository")?
  };

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path)?;

  // Remove the root branch
  if repo_state.remove_root(branch) {
    // Save the state
    repo_state.save(&repo_path)?;
    print_success(&format!("Removed {branch} from root branches",));
  } else {
    print_warning(&format!("Root branch {branch} not found",));
  }

  Ok(())
}

/// Handle the root list command
fn handle_root_list_command(list_matches: &clap::ArgMatches) -> Result<()> {
  // Get the repository path
  let repo_path = if let Some(repo_arg) = list_matches.get_one::<String>("repo") {
    crate::utils::resolve_repository_path(Some(repo_arg.as_str()))?
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
