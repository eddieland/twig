use anyhow::Result;
use clap::{Arg, ArgAction, Command};

/// Build the CLI command structure
pub fn build_cli() -> Command {
  Command::new("twig")
    .about("A Git-based developer productivity tool")
    .version(env!("CARGO_PKG_VERSION"))
    .subcommand_required(false)
    .subcommand(Command::new("init").about("Initialize twig configuration"))
    .subcommand(
      Command::new("git")
        .about("Git repository management")
        .alias("g")
        .subcommand(
          Command::new("add")
            .about("Add a repository to the registry")
            .arg(Arg::new("path").help("Path to the repository").default_value(".")),
        )
        .subcommand(
          Command::new("remove")
            .about("Remove a repository from the registry")
            .alias("rm")
            .arg(Arg::new("path").help("Path to the repository").default_value(".")),
        )
        .subcommand(
          Command::new("list")
            .about("List all repositories in the registry")
            .alias("ls"),
        )
        .subcommand(
          Command::new("fetch")
            .about("Fetch updates for repositories")
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
        ),
    )
    .subcommand(
      Command::new("worktree")
        .about("Worktree management")
        .alias("wt")
        .subcommand(
          Command::new("create")
            .about("Create a new worktree for a branch")
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
          Command::new("clean").about("Clean up stale worktrees").arg(
            Arg::new("repo")
              .long("repo")
              .short('r')
              .help("Path to a specific repository")
              .value_name("PATH"),
          ),
        ),
    )
}

/// Handle the CLI commands
pub fn handle_commands(matches: &clap::ArgMatches) -> Result<()> {
  match matches.subcommand() {
    Some(("init", _)) => crate::config::init(),
    Some(("git", git_matches)) => match git_matches.subcommand() {
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
      _ => {
        use crate::utils::output::{format_command, print_warning};
        print_warning("Unknown git command.");
        println!("Use {} for usage information.", format_command("--help"));
        Ok(())
      }
    },
    Some(("worktree", worktree_matches)) => match worktree_matches.subcommand() {
      Some(("create", create_matches)) => {
        let branch = create_matches.get_one::<String>("branch").unwrap();
        let repo_arg = create_matches.get_one::<String>("repo").map(|s| s.as_str());
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::worktree::create_worktree(repo_path, branch)?;
        Ok(())
      }
      Some(("list", list_matches)) => {
        let repo_arg = list_matches.get_one::<String>("repo").map(|s| s.as_str());
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::worktree::list_worktrees(repo_path)
      }
      Some(("clean", clean_matches)) => {
        let repo_arg = clean_matches.get_one::<String>("repo").map(|s| s.as_str());
        let repo_path = crate::utils::resolve_repository_path(repo_arg)?;
        crate::worktree::clean_worktrees(repo_path)
      }
      _ => {
        use crate::utils::output::{format_command, print_warning};
        print_warning("Unknown worktree command.");
        println!("Use {} for usage information.", format_command("--help"));
        Ok(())
      }
    },
    _ => {
      use crate::utils::output::{format_command, print_info};
      print_info("No command specified.");
      println!("Use {} for usage information.", format_command("--help"));
      Ok(())
    }
  }
}
