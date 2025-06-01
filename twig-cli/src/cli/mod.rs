//! # Command Line Interface
//!
//! Defines the CLI structure and command handlers for the twig tool,
//! including subcommands for branch management, Git operations, and
//! integrations.

mod branch;
pub mod cascade;
mod completion;
mod creds;
mod dashboard;
mod git;
mod github;
mod jira;
pub mod rebase;
mod switch;
mod sync;
mod tree;
mod worktree;

use anyhow::Result;
use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{ArgAction, Parser, Subcommand};

use crate::diagnostics;
use crate::utils::output::ColorMode;

/// Top-level CLI command for the twig tool
#[derive(Parser)]
#[command(name = "twig")]
#[command(display_name = "🌿 Twig")]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(about = "A Git-based developer productivity tool")]
#[command(
  long_about = "Twig helps developers manage multiple Git repositories and worktrees efficiently.\n\n\
        It provides commands for repository tracking, batch operations, and worktree\n\
        management to streamline your development workflow."
)]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(propagate_version = true)]
#[command(subcommand_required(true))]
#[command(disable_help_subcommand = true)]
#[command(max_term_width = 120)]
#[command(styles = Styles::styled()
    .header(AnsiColor::BrightGreen.on_default().bold().underline())
    .usage(AnsiColor::Green.on_default().bold())  // Make usage line stand out
    .literal(AnsiColor::BrightGreen.on_default().bold())  // Command names, flags bold
    .placeholder(AnsiColor::BrightWhite.on_default().italic())
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::BrightRed.on_default().bold())
)]
pub struct Cli {
  /// Sets the level of verbosity (can be used multiple times)
  #[arg(
    short = 'v',
    long = "verbose",
    action = ArgAction::Count,
    long_help = "Sets the level of verbosity for tracing and logging output.\n\n\
             -v: Show info level messages\n\
             -vv: Show debug level messages\n\
             -vvv: Show trace level messages"
  )]
  pub verbose: u8,

  /// Controls when colored output is used
  #[arg(
    long,
    value_enum,
    ignore_case = true,
    default_value_t = ColorMode::Auto,
  )]
  pub colors: ColorMode,

  /// Subcommands
  #[command(subcommand)]
  pub command: Commands,
}

/// Subcommands for the twig tool
#[derive(Subcommand)]
pub enum Commands {
  /// Branch dependency and root management
  #[command(long_about = "Manage custom branch dependencies and root branches.\n\n\
            This command group allows you to define custom parent-child relationships\n\
            between branches beyond Git's automatic detection. You can also manage\n\
            which branches should be treated as root branches in the tree view.")]
  #[command(alias = "br")]
  Branch(branch::BranchArgs),

  /// Perform a cascading rebase from the current branch to its children
  #[command(
    long_about = "Perform a cascading rebase from the current branch to its children.\n\n\
            This command rebases all child branches on their parent(s) in a cascading manner,\n\
            starting from the current branch and working down the dependency tree."
  )]
  #[command(alias = "casc")]
  Cascade(cascade::CascadeArgs),

  /// Generate shell completions
  #[command(long_about = "Generates shell completion scripts for twig commands.\n\n\
            This command generates completion scripts that provide tab completion for twig\n\
            commands and options in your shell. Supported shells include bash, zsh, and fish.")]
  Completion(completion::CompletionArgs),

  /// Credential management
  #[command(long_about = "Manage credentials for external services like Jira and GitHub.\n\n\
            This command group helps you check and set up credentials for the\n\
            external services that twig integrates with. Credentials are stored\n\
            in your .netrc file for security and compatibility with other tools.")]
  #[command(arg_required_else_help = true)]
  Creds(creds::CredsArgs),

  /// Show a comprehensive dashboard of local branches, PRs, and issues
  #[command(
    long_about = "Show a comprehensive dashboard of local branches, PRs, and issues.\n\n\
            This command displays a unified view of your development context,\n\
            including local branches, associated pull requests, and related Jira issues.\n\
            It helps you keep track of your work across different systems.\n\n\
            By default, only local branches are shown. Use --include-remote to include remote branches.\n\n\
            Use --no-github or --no-jira to disable GitHub or Jira API requests respectively.\n\
            Use --simple for a basic view that shows only branches without making any API requests."
  )]
  #[command(alias = "dash")]
  #[command(alias = "v")]
  Dashboard(dashboard::DashboardArgs),

  /// Run system diagnostics
  #[command(
    long_about = "Runs comprehensive system diagnostics to check twig's configuration and dependencies.\n\n\
            This command checks system information, configuration directories, credentials,\n\
            git configuration, tracked repositories, and network connectivity. Use this\n\
            command to troubleshoot issues or verify that twig is properly configured."
  )]
  #[command(alias = "diag")]
  Diagnostics,

  /// Git repository management
  #[command(long_about = "Manage multiple Git repositories through twig.\n\n\
            This command group allows you to register, track, and perform operations\n\
            across multiple repositories. Repositories added to twig can be referenced\n\
            in other commands and batch operations.")]
  #[command(alias = "g")]
  Git(git::GitArgs),

  /// GitHub integration
  #[command(name = "github")]
  #[command(long_about = "Interact with GitHub repositories and pull requests.\n\n\
            This command group provides functionality for working with GitHub,\n\
            including checking authentication, viewing pull request status,\n\
            and linking branches to pull requests.")]
  #[command(alias = "gh")]
  GitHub(github::GitHubArgs),

  /// Initialize twig configuration
  #[command(long_about = "Initializes the twig configuration for your environment.\n\n\
            This creates necessary configuration files in your home directory to track\n\
            repositories and store settings. Run this command once before using other\n\
            twig features. No credentials are required for this operation.")]
  Init,

  /// Jira integration
  #[command(long_about = "Interact with Jira issues and create branches from them.\n\n\
            This command group provides functionality for working with Jira,\n\
            including viewing issues, transitioning issues through workflows,\n\
            and creating branches from issues.")]
  Jira(jira::JiraArgs),

  /// Intentionally panic (for testing error handling)
  #[command(hide = true)]
  Panic,

  /// Rebase the current branch on its parent(s)
  #[command(long_about = "Rebase the current branch on its parent(s).\n\n\
            This command rebases the current branch on its parent(s) based on\n\
            the dependency tree. It can optionally start from the root branch.")]
  #[command(alias = "rb")]
  Rebase(rebase::RebaseArgs),

  /// Magic branch switching
  #[command(long_about = "Intelligently switch to branches based on various inputs.\n\n\
            This command can switch branches based on:\n\
            • Jira issue key (e.g., PROJ-123)\n\
            • Jira issue URL\n\
            • GitHub PR ID (e.g., 12345 or PR#12345)\n\
            • GitHub PR URL\n\
            • Branch name\n\n\
            The command will automatically detect the input type and find the\n\
            corresponding branch. By default, missing branches will be created\n\
            automatically. Use --no-create to disable this behavior.")]
  #[command(alias = "sw")]
  Switch(switch::SwitchArgs),

  /// Automatically link branches to Jira issues and GitHub PRs
  #[command(
    long_about = "Scan local branches and automatically detect and link them to their corresponding\n\
            Jira issues and GitHub PRs.\n\n\
            For GitHub PRs, this command:\n\
            • First searches GitHub's API for pull requests matching the branch name\n\
            • Falls back to detecting patterns in branch names if API is unavailable\n\n\
            For Jira issues, it looks for patterns in branch names like:\n\
            • PROJ-123/feature-name, feature/PROJ-123-description\n\n\
            GitHub PR branch naming patterns (fallback detection):\n\
            • pr-123-description, github-pr-123, pull-123, pr/123\n\n\
            It will automatically create associations for detected patterns and report\n\
            any branches that couldn't be linked."
  )]
  Sync(sync::SyncArgs),

  /// Show your branch tree with user-defined dependencies
  #[command(
    long_about = "Display local branches in a tree-like view based on user-defined dependencies.\n\n\
            This command shows branch relationships that you have explicitly defined using\n\
            the 'twig branch depend' command. It also displays associated Jira issues and\n\
            GitHub PRs. Branches without defined dependencies or root status will be shown\n\
            as orphaned branches. Use 'twig branch depend' to create relationships and\n\
            'twig branch root add' to designate root branches."
  )]
  #[command(alias = "t")]
  Tree(tree::TreeArgs),

  /// Worktree management
  #[command(long_about = "Manage Git worktrees for efficient multi-branch development.\n\n\
            Worktrees allow you to check out multiple branches simultaneously in separate\n\
            directories, all connected to the same repository. This enables working on\n\
            different features or fixes concurrently without stashing or committing\n\
            incomplete work.")]
  #[command(alias = "wt")]
  Worktree(worktree::WorktreeArgs),
}

pub fn handle_cli(cli: Cli) -> Result<()> {
  // Set global color override based on --colors argument
  match cli.colors {
    ColorMode::Always | ColorMode::Yes => owo_colors::set_override(true),
    ColorMode::Never | ColorMode::No => owo_colors::set_override(false),
    ColorMode::Auto => {
      // Let owo_colors use its default auto-detection
      // Don't call set_override, allowing it to detect terminal automatically
    }
  }

  match cli.command {
    Commands::Branch(branch) => branch::handle_branch_command(branch),
    Commands::Cascade(cascade) => cascade::handle_cascade_command(cascade),
    Commands::Completion(completion) => completion::handle_completion_command(completion),
    Commands::Creds(creds) => creds::handle_creds_command(creds),
    Commands::Dashboard(dashboard) => dashboard::handle_dashboard_command(dashboard),
    Commands::Diagnostics => diagnostics::run_diagnostics(),
    Commands::Git(git) => git::handle_git_command(git),
    Commands::GitHub(github) => github::handle_github_command(github),
    Commands::Init => crate::config::init(),
    Commands::Jira(jira) => jira::handle_jira_command(jira),
    Commands::Panic => {
      panic!("This is an intentional test panic to verify no-worries integration");
    }
    Commands::Rebase(rebase) => rebase::handle_rebase_command(rebase),
    Commands::Switch(switch) => switch::handle_switch_command(switch),
    Commands::Sync(sync) => sync::handle_sync_command(sync),
    Commands::Tree(tree) => tree::handle_tree_command(tree),
    Commands::Worktree(worktree) => worktree::handle_worktree_command(worktree),
  }
}
