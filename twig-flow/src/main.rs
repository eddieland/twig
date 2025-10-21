//! twig-flow: canonical branch flow plugin for twig.
//!
//! This plugin demonstrates how to build a first-class twig plugin in Rust by
//! combining branch tree visualization with smart branch switching. The
//! implementation reuses shared functionality from `twig-core` so that the
//! plugin stays in sync with the CLI experience.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{ArgAction, Parser};
use directories::BaseDirs;
use git2::Repository as Git2Repository;
use tracing::Level;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};
use twig_core::clients::{create_github_client_from_netrc, create_jira_client_from_netrc, get_jira_host};
use twig_core::flow::switch::{
  InputType, detect_input_type, handle_branch_switch, handle_github_pr_switch, handle_jira_switch, handle_root_switch,
};
use twig_core::output::{format_command, print_info, print_warning};
use twig_core::state::RepoState;
use twig_core::tree_renderer::TreeRenderer;
use twig_core::user_defined_dependency_resolver::UserDefinedDependencyResolver;
use twig_core::{create_jira_parser, detect_repository};

/// CLI for the twig-flow plugin.
#[derive(Parser)]
#[command(name = "twig-flow")]
#[command(about = "Navigate git branch flows with tree context and smart switching")]
#[command(version, disable_help_subcommand = true)]
struct FlowCli {
  /// Optional branch, Jira issue, or GitHub PR input.
  #[arg(index = 1)]
  input: Option<String>,

  /// Jump to the dependency tree root for the current branch.
  #[arg(long = "root")]
  root: bool,

  /// Do not create missing branches.
  #[arg(long = "no-create")]
  no_create: bool,

  /// Configure the parent relationship when creating a new branch.
  #[arg(
    short,
    long,
    value_name = "PARENT",
    num_args = 0..=1,
    default_missing_value = "current",
    long_help = "Set parent dependency for a created branch. Use 'current', a branch name, a Jira issue key, or 'none'."
  )]
  parent: Option<String>,

  /// Render the tree from this repository path instead of detecting
  /// automatically.
  #[arg(long = "repo", value_name = "PATH")]
  repo: Option<PathBuf>,

  /// Limit tree depth when rendering the branch flow.
  #[arg(long = "max-depth", short = 'd')]
  max_depth: Option<u32>,

  /// Disable colorized tree output.
  #[arg(long = "no-color")]
  no_color: bool,

  /// Increase local verbosity (fallback for TWIG_VERBOSITY).
  #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
  verbose: u8,
}

fn level_to_count(level: Level) -> u8 {
  match level {
    Level::TRACE => 3,
    Level::DEBUG => 2,
    Level::INFO => 1,
    _ => 0,
  }
}

fn init_tracing(default_level: Level) -> Result<()> {
  let verbosity = env::var("TWIG_VERBOSITY")
    .ok()
    .and_then(|v| v.parse::<u8>().ok())
    .filter(|level| *level <= 3)
    .unwrap_or(level_to_count(default_level));

  let level = match verbosity {
    0 => Level::WARN,
    1 => Level::INFO,
    2 => Level::DEBUG,
    _ => Level::TRACE,
  };

  let fmt_layer = fmt::layer().with_target(false).with_level(true);
  let filter = EnvFilter::default().add_directive(level.into());

  tracing_subscriber::registry()
    .with(filter)
    .with(fmt_layer)
    .try_init()
    .ok();
  Ok(())
}

fn main() -> Result<()> {
  let cli = FlowCli::parse();
  init_tracing(match cli.verbose {
    0 => Level::WARN,
    1 => Level::INFO,
    2 => Level::DEBUG,
    _ => Level::TRACE,
  })?;

  let repo_path = if let Some(path) = &cli.repo {
    path.clone()
  } else {
    detect_repository().context("Not in a git repository")?
  };

  if cli.input.is_none() && !cli.root {
    return render_branch_tree(&repo_path, cli.max_depth, cli.no_color);
  }

  let create_if_missing = !cli.no_create;
  let parent_option = cli.parent.as_deref();
  let jira_parser = create_jira_parser();

  if cli.root {
    if cli.input.is_some() {
      return Err(anyhow!(
        "Cannot specify both --root and an input. Use one or the other."
      ));
    }
    return handle_root_switch(&repo_path);
  }

  let input = cli
    .input
    .as_deref()
    .ok_or_else(|| anyhow!("No input provided. Provide a branch, Jira issue, or GitHub PR."))?;

  match detect_input_type(jira_parser.as_ref(), input) {
    InputType::JiraIssueKey(issue_key) | InputType::JiraIssueUrl(issue_key) => {
      let jira_host = get_jira_host().context("Failed to determine Jira host")?;
      let base_dirs = BaseDirs::new().context("Failed to resolve home directory")?;
      let jira = create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host)
        .context("Failed to create Jira client from credentials")?;

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
      let base_dirs = BaseDirs::new().context("Failed to resolve home directory")?;
      let gh = create_github_client_from_netrc(base_dirs.home_dir())
        .context("Failed to create GitHub client from credentials")?;

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

fn render_branch_tree(repo_path: &PathBuf, max_depth: Option<u32>, no_color: bool) -> Result<()> {
  let repo = Git2Repository::open(repo_path)
    .with_context(|| format!("Failed to open git repository at {}", repo_path.display()))?;
  let repo_state = RepoState::load(repo_path).unwrap_or_default();
  let resolver = UserDefinedDependencyResolver;
  let branch_nodes = resolver.resolve_user_dependencies(&repo, &repo_state)?;

  if branch_nodes.is_empty() {
    print_warning("No local branches found.");
    return Ok(());
  }

  let has_dependencies = repo_state.has_user_defined_dependencies();
  let has_root_branches = !repo_state.get_root_branches().is_empty();

  if !has_dependencies && !has_root_branches {
    display_empty_state_help();
    return Ok(());
  }

  let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

  if roots.is_empty() {
    display_no_roots_warning(&branch_nodes);
    return Ok(());
  }

  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, max_depth, no_color);
  let mut stdout = std::io::stdout();
  renderer.render(&mut stdout, &roots, Some("\n"))?;

  if !orphaned.is_empty() {
    display_orphaned_branches(&orphaned);
  }

  display_summary(&branch_nodes);
  Ok(())
}

fn display_summary(branch_nodes: &std::collections::HashMap<String, twig_core::tree_renderer::BranchNode>) {
  let branches_with_issues = branch_nodes.values().filter(|node| node.metadata.is_some()).count();
  let branches_with_prs = branch_nodes
    .values()
    .filter(|node| {
      node
        .metadata
        .as_ref()
        .map(|issue| issue.github_pr.is_some())
        .unwrap_or(false)
    })
    .count();

  if branches_with_issues == 0 && branches_with_prs == 0 {
    println!();
    print_info("To associate branches with issues and PRs:");
    println!(
      "  • Link Jira issues: {}",
      format_command("twig jira branch link <issue-key>")
    );
    println!(
      "  • Link GitHub PRs: {}",
      format_command("twig github pr link <pr-url>")
    );
  }
}

fn display_empty_state_help() {
  print_info("No user-defined dependencies or root branches found.");
  println!("\nTo get started with branch dependencies:");
  println!(
    "  • Define root branches: {}",
    format_command("twig branch root add <branch-name>")
  );
  println!(
    "  • Add dependencies: {}",
    format_command("twig branch depend <parent-branch>")
  );
  println!("  • View current setup: {}", format_command("twig branch list"));
  println!("\nThis will create a tree structure showing your branch relationships.");
}

fn display_no_roots_warning(branch_nodes: &std::collections::HashMap<String, twig_core::tree_renderer::BranchNode>) {
  print_warning("Found user-defined dependencies but no root branches.");

  let branch_names: Vec<&String> = branch_nodes.keys().collect();
  println!("\nAvailable branches:");
  for name in &branch_names {
    println!("  {name}");
  }

  println!("\nTo fix this, designate one or more root branches:");
  println!("  {}", format_command("twig branch root add <branch-name>"));
}

fn display_orphaned_branches(orphaned: &[String]) {
  println!("\n📝 Orphaned branches (no dependencies defined):");
  for branch in orphaned {
    println!("  • {branch}");
  }

  println!("\nTo organize these branches:");
  println!(
    "  • Add as root: {}",
    format_command("twig branch root add <branch-name>")
  );
  println!(
    "  • Add dependency: {}",
    format_command("twig branch depend <parent-branch>")
  );
}
