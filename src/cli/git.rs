use anyhow::Result;
use clap::{Arg, ArgAction, Command};

/// Build the git subcommand
pub fn build_command() -> Command {
  Command::new("git")
    .about("Git repository management")
    .long_about(
      "Manage multiple Git repositories through twig.\n\n\
            This command group allows you to register, track, and perform operations\n\
            across multiple repositories. Repositories added to twig can be referenced\n\
            in other commands and batch operations.",
    )
    .arg_required_else_help(true)
    .alias("g")
    .subcommand(
      Command::new("add")
        .about("Add a repository to the registry")
        .long_about(
          "Registers a Git repository with twig for tracking and management.\n\n\
                    Once added, the repository can be referenced in other twig commands and\n\
                    included in batch operations. The repository must be a valid Git repository\n\
                    with proper credentials configured if needed for remote operations.",
        )
        .arg(Arg::new("path").help("Path to the repository").default_value(".")),
    )
    .subcommand(
      Command::new("remove")
        .about("Remove a repository from the registry")
        .long_about(
          "Removes a previously registered Git repository from twig's tracking.\n\n\
                    This only affects twig's registry and does not delete or modify the\n\
                    actual repository files.",
        )
        .alias("rm")
        .arg(Arg::new("path").help("Path to the repository").default_value(".")),
    )
    .subcommand(
      Command::new("list")
        .about("List all repositories in the registry")
        .long_about(
          "Displays all Git repositories currently registered with twig.\n\n\
                    Shows the repository paths and any additional tracking information\n\
                    to help you manage your repositories.",
        )
        .alias("ls"),
    )
    .subcommand(
      Command::new("fetch")
        .about("Fetch updates for repositories")
        .long_about(
          "Fetches updates from remote repositories.\n\n\
                    This updates local references to remote branches without modifying your\n\
                    working directory. Requires proper Git credentials to be configured for\n\
                    repositories with private remotes.",
        )
        .arg(
          Arg::new("all")
            .long("all")
            .short('a')
            .help("Fetch all repositories in the registry")
            .action(ArgAction::SetTrue),
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
      Command::new("exec")
        .about("Execute a git command in repositories")
        .long_about(
          "Executes a Git command in one or all registered repositories.\n\n\
                    This powerful feature allows you to run the same Git operation across\n\
                    multiple repositories simultaneously. The command is executed as-is,\n\
                    so ensure it's a valid Git command. Credentials may be required\n\
                    depending on the Git operation being performed.",
        )
        .arg(
          Arg::new("all")
            .long("all")
            .short('a')
            .help("Execute in all repositories in the registry")
            .action(ArgAction::SetTrue),
        )
        .arg(
          Arg::new("repo")
            .long("repo")
            .short('r')
            .help("Path to a specific repository")
            .value_name("PATH"),
        )
        .arg(Arg::new("command").help("Command to execute").required(true).index(1)),
    )
    .subcommand(
      Command::new("stale-branches")
        .about("List stale branches in repositories")
        .long_about(
          "Identifies and lists branches that haven't been updated recently.\n\n\
                    This helps you identify abandoned or forgotten branches that might need\n\
                    attention, cleanup, or merging. The command analyzes local branch information\n\
                    and doesn't require remote credentials unless combined with a fetch operation.",
        )
        .alias("stale")
        .arg(
          Arg::new("days")
            .long("days")
            .short('d')
            .help("Number of days to consider a branch stale")
            .value_name("DAYS")
            .default_value("30"),
        )
        .arg(
          Arg::new("all")
            .long("all")
            .short('a')
            .help("Check all repositories in the registry")
            .action(ArgAction::SetTrue),
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

/// Handle git subcommands
pub fn handle_commands(git_matches: &clap::ArgMatches) -> Result<()> {
  match git_matches.subcommand() {
    Some(("add", add_matches)) => {
      let path = add_matches.get_one::<String>("path").unwrap();
      crate::git::add_repository(path)
    }
    Some(("remove", rm_matches)) => {
      let path = rm_matches.get_one::<String>("path").unwrap();
      crate::git::remove_repository(path)
    }
    Some(("list", _)) => crate::git::list_repositories(),
    Some(("fetch", fetch_matches)) => {
      if fetch_matches.get_flag("all") {
        crate::git::fetch_all_repositories()
      } else {
        let repo_arg = fetch_matches.get_one::<String>("repo").map(|s| s.as_str());
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::git::fetch_repository(repo_path, true)
      }
    }
    Some(("exec", exec_matches)) => {
      let command = exec_matches.get_one::<String>("command").unwrap();

      if exec_matches.get_flag("all") {
        crate::git::execute_all_repositories(command)
      } else {
        let repo_arg = exec_matches.get_one::<String>("repo").map(|s| s.as_str());
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::git::execute_repository(repo_path, command)
      }
    }
    Some(("stale-branches", stale_matches)) => {
      let days = stale_matches
        .get_one::<String>("days")
        .unwrap()
        .parse::<u32>()
        .map_err(|e| anyhow::anyhow!("Days must be a positive number: {}", e))?;

      if stale_matches.get_flag("all") {
        crate::git::find_stale_branches_all(days)
      } else {
        let repo_arg = stale_matches.get_one::<String>("repo").map(|s| s.as_str());
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::git::find_stale_branches(repo_path, days)
      }
    }
    _ => {
      use crate::utils::output::print_warning;
      print_warning("Unknown git command.");
      // Print the help text directly instead of telling the user to use --help
      let mut cmd = build_command();
      cmd.print_help().expect("Failed to print help text");
      println!();
      Ok(())
    }
  }
}
