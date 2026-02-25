//! # Adopt Command
//!
//! Re-parent orphaned branches by attaching them to a chosen parent branch.
//! Supports three modes:
//! - Automatic dependency resolution (default)
//! - Attach to the default root branch
//! - Attach to a specific branch
//!
//! The command always previews the proposed tree and asks for confirmation
//! before making any changes.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::{Args, ValueEnum};
use git2::Repository as Git2Repository;
use tracing::debug;
use tree_renderer::TreeRenderer;
use twig_core::output::{print_info, print_success, print_warning};
use twig_core::{RepoState, detect_repository, tree_renderer};

use crate::auto_dependency_discovery::AutoDependencyDiscovery;
use crate::complete::branch_completer;
use crate::user_defined_dependency_resolver::UserDefinedDependencyResolver;

/// Modes for the adopt command
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum AdoptMode {
  /// Use automatic dependency resolver (default)
  Auto,
  /// Attach all orphans to the default root branch
  DefaultRoot,
  /// Attach all orphans to a specific branch
  Branch,
}

/// Arguments for the adopt command
#[derive(Args)]
pub struct AdoptArgs {
  /// Path to a specific repository
  #[arg(short, long, value_name = "PATH")]
  pub repo: Option<String>,

  /// Adoption mode
  #[arg(long, value_enum, default_value_t = AdoptMode::Auto)]
  pub mode: AdoptMode,

  /// Branch to adopt orphaned branches under (implies --mode branch)
  #[arg(long, value_name = "BRANCH", add = branch_completer())]
  pub parent: Option<String>,

  /// Automatically confirm adoption without prompting
  #[arg(short = 'y', long)]
  pub yes: bool,

  /// Maximum depth to display in the preview tree
  #[arg(short = 'd', long = "max-depth", value_name = "DEPTH")]
  pub max_depth: Option<u32>,

  /// Disable colored output in the preview tree
  #[arg(long = "no-color")]
  pub no_color: bool,
}

#[derive(Clone, Debug)]
struct AdoptionPlan {
  child: String,
  parent: String,
  reason: String,
}

/// Handle the adopt command
pub(crate) fn handle_adopt_command(args: AdoptArgs) -> Result<()> {
  let repo_path = if let Some(repo_arg) = &args.repo {
    PathBuf::from(repo_arg)
  } else {
    detect_repository().context("Not in a git repository")?
  };

  debug!(?repo_path, "Starting twig adopt command");

  let repo =
    Git2Repository::open(&repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  let mut repo_state =
    RepoState::load(&repo_path).with_context(|| format!("Failed to load repo state at {}", repo_path.display()))?;

  debug!(
    branch_metadata_count = repo_state.branches.len(),
    dependency_count = repo_state.dependencies.len(),
    root_branch_count = repo_state.root_branches.len(),
    "Loaded repository state"
  );

  let user_resolver = UserDefinedDependencyResolver;
  let branch_nodes = user_resolver.resolve_user_dependencies_without_default_root(&repo, &repo_state)?;
  let (_, orphaned) = user_resolver.build_tree_from_user_dependencies(&branch_nodes, &repo_state);

  debug!(
    branch_node_count = branch_nodes.len(),
    orphaned_count = orphaned.len(),
    "Resolved user-defined dependencies"
  );

  if orphaned.is_empty() {
    print_info("No orphaned branches found. Nothing to adopt.");
    return Ok(());
  }

  let mode = if args.parent.is_some() {
    AdoptMode::Branch
  } else {
    args.mode
  };
  if mode == AdoptMode::Branch && args.parent.is_none() {
    return Err(anyhow!("--parent must be provided when using --mode branch"));
  }

  debug!(?mode, parent = ?args.parent, "Building adoption plan");
  let plan = build_adoption_plan(mode, &args.parent, &orphaned, &repo_state, &branch_nodes, &repo)?;
  debug!(plan_count = plan.len(), "Finished building adoption plan");

  if plan.is_empty() {
    print_warning("No adoption suggestions could be generated for the orphaned branches.");
    return Ok(());
  }

  display_plan(&plan);

  let mut preview_state = repo_state.clone();
  apply_plan(&mut preview_state, &plan)?;

  render_preview_tree(&repo, &preview_state, &args)?;

  if !args.yes && !crate::utils::prompt_for_confirmation("Apply this adoption plan?")? {
    print_info("Aborted without making changes.");
    return Ok(());
  }

  debug!(plan_count = plan.len(), "Applying adoption plan to repository state");
  apply_plan(&mut repo_state, &plan)?;
  repo_state.save(&repo_path)?;

  print_success("Adoption complete. Branch relationships updated.");
  Ok(())
}

fn build_adoption_plan(
  mode: AdoptMode,
  parent_override: &Option<String>,
  orphaned: &[String],
  repo_state: &RepoState,
  branch_nodes: &HashMap<String, tree_renderer::BranchNode>,
  repo: &Git2Repository,
) -> Result<Vec<AdoptionPlan>> {
  match mode {
    AdoptMode::DefaultRoot => adopt_to_default_root(orphaned, repo_state),
    AdoptMode::Branch => {
      let parent = parent_override
        .as_ref()
        .ok_or_else(|| anyhow!("A parent branch must be specified for branch mode"))?;
      adopt_to_specific_parent(orphaned, parent, branch_nodes)
    }
    AdoptMode::Auto => adopt_with_auto_resolver(orphaned, repo_state, branch_nodes, repo),
  }
}

fn adopt_to_default_root(orphaned: &[String], repo_state: &RepoState) -> Result<Vec<AdoptionPlan>> {
  let Some(default_root) = repo_state.get_default_root() else {
    return Err(anyhow!(
      "No default root is configured. Set one with 'twig branch root add <branch> --default'."
    ));
  };

  Ok(
    orphaned
      .iter()
      .map(|child| AdoptionPlan {
        child: child.clone(),
        parent: default_root.to_string(),
        reason: "Attach to default root".to_string(),
      })
      .collect(),
  )
}

fn adopt_to_specific_parent(
  orphaned: &[String],
  parent: &str,
  branch_nodes: &HashMap<String, tree_renderer::BranchNode>,
) -> Result<Vec<AdoptionPlan>> {
  if !branch_nodes.contains_key(parent) {
    return Err(anyhow!("Parent branch '{parent}' does not exist locally."));
  }

  Ok(
    orphaned
      .iter()
      .map(|child| AdoptionPlan {
        child: child.clone(),
        parent: parent.to_string(),
        reason: "Attach to specified parent".to_string(),
      })
      .collect(),
  )
}

fn adopt_with_auto_resolver(
  orphaned: &[String],
  repo_state: &RepoState,
  branch_nodes: &HashMap<String, tree_renderer::BranchNode>,
  repo: &Git2Repository,
) -> Result<Vec<AdoptionPlan>> {
  debug!(
    orphaned_count = orphaned.len(),
    branch_node_count = branch_nodes.len(),
    dependency_count = repo_state.dependencies.len(),
    "Running auto dependency resolver for adoption"
  );

  let discovery = AutoDependencyDiscovery;
  let suggestions = discovery.suggest_dependencies(repo, repo_state)?;
  debug!(
    suggestion_count = suggestions.len(),
    "Auto dependency suggestions generated"
  );

  let mut best_suggestions: HashMap<&str, &crate::auto_dependency_discovery::DependencySuggestion> = HashMap::new();
  for suggestion in &suggestions {
    if let Some(existing) = best_suggestions.get(suggestion.child.as_str()) {
      let ordering = suggestion
        .confidence
        .partial_cmp(&existing.confidence)
        .unwrap_or(Ordering::Equal);
      if ordering == Ordering::Greater {
        best_suggestions.insert(&suggestion.child, suggestion);
      }
    } else {
      best_suggestions.insert(&suggestion.child, suggestion);
    }
  }

  let resolver = UserDefinedDependencyResolver;
  let fallback_parent = resolver.get_or_suggest_default_root(repo_state, branch_nodes);

  let mut plan = Vec::new();
  for child in orphaned {
    if let Some(suggestion) = best_suggestions.get(child.as_str()) {
      plan.push(AdoptionPlan {
        child: child.clone(),
        parent: suggestion.parent.clone(),
        reason: suggestion.reason.clone(),
      });
      continue;
    }

    if let Some(fallback) = &fallback_parent {
      plan.push(AdoptionPlan {
        child: child.clone(),
        parent: fallback.clone(),
        reason: "Fallback to suggested root".to_string(),
      });
    }
  }

  if plan.len() < orphaned.len() {
    print_warning("Some orphaned branches could not be matched automatically.");
  }

  Ok(plan)
}

fn apply_plan(repo_state: &mut RepoState, plan: &[AdoptionPlan]) -> Result<()> {
  for adoption in plan {
    repo_state
      .add_dependency(adoption.child.clone(), adoption.parent.clone())
      .with_context(|| format!("Failed to add dependency {} -> {}", adoption.child, adoption.parent))?;
  }

  Ok(())
}

fn render_preview_tree(repo: &Git2Repository, preview_state: &RepoState, args: &AdoptArgs) -> Result<()> {
  let resolver = UserDefinedDependencyResolver;
  let branch_nodes = resolver.resolve_user_dependencies(repo, preview_state)?;
  let (roots, orphaned) = resolver.build_tree_from_user_dependencies(&branch_nodes, preview_state);

  if roots.is_empty() {
    print_warning("No root branches available after adoption plan.");
    return Ok(());
  }

  let mut renderer = TreeRenderer::new(&branch_nodes, &roots, args.max_depth, args.no_color);
  let mut stdout = io::stdout();

  println!("\nProposed tree (no changes made yet):\n");
  renderer.render(&mut stdout, &roots, Some("\n"))?;

  if !orphaned.is_empty() {
    println!("\nRemaining orphaned branches after adoption plan:");
    for branch in orphaned {
      println!("  • {branch}");
    }
  }

  Ok(())
}

fn display_plan(plan: &[AdoptionPlan]) {
  println!("Adoption plan (proposed dependencies):");
  for adoption in plan {
    println!("  • {} -> {} ({})", adoption.child, adoption.parent, adoption.reason);
  }
}

