//! # Command Line Interface
//!
//! Defines the CLI structure and command handlers for the twig tool,
//! including subcommands for branch management, Git operations, and
//! integrations.

use anyhow::Result;
use clap::{Arg, ArgAction, Command};

pub mod derive;

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
    .arg_required_else_help(true)
    .arg(
      Arg::new("verbose")
        .short('v')
        .long("verbose")
        .action(ArgAction::Count)
        .help("Sets the level of verbosity (can be used multiple times)")
        .long_help(
          "Sets the level of verbosity for tracing and logging output.\n\
                 -v: Show info level messages\n\
                 -vv: Show debug level messages\n\
                 -vvv: Show trace level messages",
        ),
    )
    .arg(
      Arg::new("colors")
        .long("colors")
        .help("When to use colored output")
        .long_help(
          "Controls when colored output is used.\n\
                 yes: Always use colors\n\
                 auto: Use colors when outputting to a terminal (default)\n\
                 no: Never use colors",
        )
        .value_parser(["yes", "auto", "no"])
        .default_value("auto"),
    )
    .subcommand(derive::init::InitCommand::command())
    .subcommand(derive::panic::PanicCommand::command())
    .subcommand(derive::branch::BranchCommand::command())
    .subcommand(derive::creds::CredsCommand::command())
    .subcommand(derive::git::GitCommand::command())
    .subcommand(derive::github::GitHubCommand::command())
    .subcommand(derive::jira::JiraCommand::command())
    .subcommand(derive::switch::SwitchCommand::command())
    .subcommand(derive::sync::SyncCommand::command())
    .subcommand(derive::tree::TreeCommand::command())
    .subcommand(derive::view::ViewCommand::command())
    .subcommand(derive::worktree::WorktreeCommand::command())
    .subcommand(derive::diagnostics::DiagnosticsCommand::command())
    .subcommand(derive::completion::CompletionCommand::command())
    .subcommand(derive::dashboard::DashboardCommand::command())
}

/// Handle the CLI commands
pub fn handle_commands(matches: &clap::ArgMatches) -> Result<()> {
  // Set global color override based on --colors argument
  match matches.get_one::<String>("colors").map(|s| s.as_str()) {
    Some("yes") => owo_colors::set_override(true),
    Some("no") => owo_colors::set_override(false),
    None => {
      // Let owo_colors use its default auto-detection
      // Don't call set_override, allowing it to detect terminal automatically
    }
    _ => {} // Should not happen due to value_parser, but just in case
  }

  match matches.subcommand() {
    Some(("init", _)) => derive::init::InitCommand::parse_and_execute(),
    Some(("panic", _)) => derive::panic::PanicCommand::parse_and_execute(),
    Some(("branch", branch_matches)) => derive::branch::BranchCommand::parse_and_execute(branch_matches),
    Some(("creds", creds_matches)) => derive::creds::CredsCommand::parse_and_execute(creds_matches),
    Some(("git", git_matches)) => derive::git::GitCommand::parse_and_execute(git_matches),
    Some(("github", github_matches)) => derive::github::GitHubCommand::parse_and_execute(github_matches),
    Some(("jira", jira_matches)) => derive::jira::JiraCommand::parse_and_execute(jira_matches),
    Some(("switch", switch_matches)) => derive::switch::SwitchCommand::parse_and_execute(switch_matches),
    Some(("sync", sync_matches)) => derive::sync::SyncCommand::parse_and_execute(sync_matches),
    Some(("tree", tree_matches)) => derive::tree::TreeCommand::parse_and_execute(tree_matches),
    Some(("view", view_matches)) => derive::view::ViewCommand::parse_and_execute(view_matches),
    Some(("worktree", worktree_matches)) => derive::worktree::WorktreeCommand::parse_and_execute(worktree_matches),
    Some(("diagnose", _)) => derive::diagnostics::DiagnosticsCommand::parse_and_execute(),
    Some(("completion", completion_matches)) => {
      derive::completion::CompletionCommand::parse_and_execute(completion_matches)
    }
    Some(("dashboard", dashboard_matches)) => derive::dashboard::DashboardCommand::parse_and_execute(dashboard_matches),
    _ => {
      use crate::utils::output::print_info;
      print_info("No command specified.");
      Ok(())
    }
  }
}
