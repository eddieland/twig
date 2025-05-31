//! # Command Line Interface
//!
//! Defines the CLI structure and command handlers for the twig tool,
//! including subcommands for branch management, Git operations, and
//! integrations.

use anyhow::Result;
use clap::{Arg, ArgAction, Command};

mod branch;
mod commands;
mod completion;
mod creds;
mod dashboard;
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
    .subcommand(dashboard::build_command())
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
    Some(("dashboard", dashboard_matches)) => dashboard::handle_command(dashboard_matches),
    _ => {
      use crate::utils::output::print_info;
      print_info("No command specified.");
      Ok(())
    }
  }
}
