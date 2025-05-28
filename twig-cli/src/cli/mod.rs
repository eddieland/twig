//! # Command Line Interface
//!
//! Defines the CLI structure and command handlers for the twig tool,
//! including subcommands for branch management, Git operations, and
//! integrations.

use anyhow::Result;
use clap::Command;

mod branch;
mod commands;
mod completion;
mod creds;
mod diagnostics;
mod git;
mod github;
mod jira;
mod switch;
mod sync;
mod tree;
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
    .subcommand(commands::build_panic_command())
    .subcommand(branch::build_command())
    .subcommand(creds::build_command())
    .subcommand(git::build_command())
    .subcommand(github::build_command())
    .subcommand(jira::build_command())
    .subcommand(switch::build_command())
    .subcommand(sync::build_command())
    .subcommand(tree::build_command())
    .subcommand(worktree::build_command())
    .subcommand(diagnostics::build_diagnostics_command())
    .subcommand(completion::build_completion_command())
}

/// Handle the CLI commands
pub fn handle_commands(matches: &clap::ArgMatches) -> Result<()> {
  match matches.subcommand() {
    Some(("init", _)) => commands::handle_init_command(),
    Some(("panic", _)) => commands::handle_panic_command(),
    Some(("branch", branch_matches)) => branch::handle_commands(branch_matches),
    Some(("creds", creds_matches)) => creds::handle_commands(creds_matches),
    Some(("git", git_matches)) => git::handle_commands(git_matches),
    Some(("github", github_matches)) => github::handle_commands(github_matches),
    Some(("jira", jira_matches)) => jira::handle_commands(jira_matches),
    Some(("switch", switch_matches)) => switch::handle_command(switch_matches),
    Some(("sync", sync_matches)) => sync::handle_command(sync_matches),
    Some(("tree", tree_matches)) => tree::handle_command(tree_matches),
    Some(("worktree", worktree_matches)) => worktree::handle_commands(worktree_matches),
    Some(("diagnose", _)) => diagnostics::handle_diagnostics_command(),
    Some(("completion", completion_matches)) => completion::handle_completion_command(completion_matches),
    _ => commands::handle_unknown_command(),
  }
}
