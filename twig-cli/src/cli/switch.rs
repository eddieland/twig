//! # Switch Command
//!
//! Wrapper around the shared branch-switching logic in `twig-core`. The CLI
//! remains responsible for argument parsing and client construction while the
//! heavy lifting lives in the core crate for reuse by plugins.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use directories::BaseDirs;
use twig_core::clients::{create_github_client_from_netrc, create_jira_client_from_netrc, get_jira_host};
use twig_core::flow::switch::{
  InputType, detect_input_type, handle_branch_switch, handle_github_pr_switch, handle_jira_switch, handle_root_switch,
};
use twig_core::{create_jira_parser, detect_repository};

/// Command for intelligently switching to branches based on various inputs
#[derive(Args)]
pub struct SwitchArgs {
  #[arg(
    required = false,
    index = 1,
    long_help = "Jira issue, GitHub PR, or branch name\n\n\
                Can be any of the following:\n\
                • Jira issue key (PROJ-123)\n\
                • Jira issue URL (https://company.atlassian.net/browse/PROJ-123)\n\
                • GitHub PR ID (12345 or PR#12345)\n\
                • GitHub PR URL (https://github.com/owner/repo/pull/123)\n\
                • Branch name (feature/my-branch)\n\n\
                Not required when using --root flag."
  )]
  pub input: Option<String>,

  #[arg(
    long = "root",
    long_help = "Switch to the current branch's dependency tree root\n\n\
                Traverses up the dependency chain from the current branch to find and switch to\n\
                the topmost parent branch. If the current branch has no dependencies, it will\n\
                remain on the current branch. This helps navigate to the root of a feature\n\
                branch dependency tree."
  )]
  pub root: bool,

  #[arg(
    long = "no-create",
    long_help = "Don't create branch if it doesn't exist\n\n\
               Disable the default behavior of creating branches when they don't exist.\n\
               By default, twig switch will create missing branches. Use this flag\n\
               to only switch to existing branches."
  )]
  pub no_create: bool,

  #[arg(
    short,
    long,
    value_name = "PARENT",
    num_args = 0..=1,
    default_missing_value = "current",
    long_help = "Set parent dependency for the new branch (only applies when creating a new branch)\n\n\
               Specify a parent branch to create a dependency relationship when a new branch is created.\n\
               This option is ignored when switching to existing branches.\n\
               Values can be:\n\
               • 'current' (default if flag used without value): Use current branch\n\
               • A branch name: Use the specified branch\n\
               • A Jira issue key (e.g., PROJ-123): Use branch associated with Jira issue\n\
               • 'none': Don't set any parent (use default root)"
  )]
  pub parent: Option<String>,
}

/// Handle the switch command by delegating to shared flow helpers in
/// `twig-core`.
pub(crate) fn handle_switch_command(switch: SwitchArgs) -> Result<()> {
  let create_if_missing = !switch.no_create;
  let parent_option = switch.parent.as_deref();

  // Get the current repository
  let repo_path = detect_repository().context("Not in a git repository")?;

  // Create Jira parser once for the entire command
  let jira_parser = create_jira_parser();

  // Handle --root flag
  if switch.root {
    if switch.input.is_some() {
      return Err(anyhow!(
        "Cannot specify both --root flag and an input argument. Use either --root or provide an input."
      ));
    }
    return handle_root_switch(&repo_path);
  }

  // Require input if --root is not specified
  let input = match switch.input.as_ref() {
    Some(input) => input,
    None => {
      return Err(anyhow!(
        "No input provided. Please specify a Jira issue, GitHub PR, or branch name.\nFor more information, run: twig switch --help"
      ));
    }
  };

  // Detect input type and handle accordingly
  match detect_input_type(jira_parser.as_ref(), input) {
    InputType::JiraIssueKey(issue_key) | InputType::JiraIssueUrl(issue_key) => {
      let jira_host = get_jira_host().context("Failed to get Jira host")?;

      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let jira =
        create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host).context("Failed to create Jira client")?;

      handle_jira_switch(
        &jira,
        &repo_path,
        &issue_key,
        create_if_missing,
        parent_option,
        jira_parser.as_ref(),
      )
    }
    InputType::GitHubPrId(pr_number) | InputType::GitHubPrUrl(pr_number) => {
      let base_dirs = BaseDirs::new().context("Failed to get $HOME")?;
      let gh = create_github_client_from_netrc(base_dirs.home_dir()).context("Failed to create GitHub client")?;

      handle_github_pr_switch(
        &gh,
        &repo_path,
        pr_number,
        create_if_missing,
        parent_option,
        jira_parser.as_ref(),
      )
    }
    InputType::BranchName(branch_name) => handle_branch_switch(
      &repo_path,
      &branch_name,
      create_if_missing,
      parent_option,
      jira_parser.as_ref(),
    ),
  }
}
