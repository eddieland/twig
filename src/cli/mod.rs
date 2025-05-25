use anyhow::Result;
use clap::Command;

mod commands;
mod creds;
mod git;
mod github;
mod jira;
mod worktree;

/// Build the CLI command structure
pub fn build_cli() -> Command {
  Command::new("twig")
    .about("A Git-based developer productivity tool")
    .long_about(
      "Twig helps developers manage multiple Git repositories and worktrees efficiently.\n\n\
            It provides commands for repository tracking, batch operations, and worktree\n\
            management to streamline your development workflow.",
    )
    .version(env!("CARGO_PKG_VERSION"))
    .subcommand_required(false)
    .subcommand(commands::build_init_command())
    .subcommand(creds::build_command())
    .subcommand(git::build_command())
    .subcommand(github::build_command())
    .subcommand(jira::build_command())
    .subcommand(worktree::build_command())
}

/// Handle the CLI commands
pub fn handle_commands(matches: &clap::ArgMatches) -> Result<()> {
  match matches.subcommand() {
    Some(("init", _)) => commands::handle_init_command(),
    Some(("creds", creds_matches)) => creds::handle_commands(creds_matches),
    Some(("git", git_matches)) => git::handle_commands(git_matches),
    Some(("github", github_matches)) => github::handle_commands(github_matches),
    Some(("jira", jira_matches)) => jira::handle_commands(jira_matches),
    Some(("worktree", worktree_matches)) => worktree::handle_commands(worktree_matches),
    _ => commands::handle_unknown_command(),
  }
}
