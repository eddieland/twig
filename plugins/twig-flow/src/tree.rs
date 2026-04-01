//! Branch-tree visualization for the `twig-flow` plugin.
//!
//! This module orchestrates [`twig_core`]'s graph, tree-algorithm, and renderer
//! primitives into the end-to-end workflow behind `twig-flow` (no `--target`):
//!
//! 1. **Repository & state loading** — [`get_repository`] locates the Git repo; [`RepoState`] provides persisted branch
//!    dependencies and root configuration from `.twig/state.json`.
//! 2. **Optional branch switching** — The `--root` and `--parent` CLI flags check out a branch before rendering,
//!    delegating to [`checkout_branch`].
//! 3. **Graph construction** — [`BranchGraphBuilder`] materialises a [`BranchGraph`] DAG from the repository's local
//!    branches and their configured dependency edges.
//! 4. **Orphan handling** — Branches without declared parents are detected by [`find_orphaned_branches`], grafted under
//!    the default root via [`attach_orphans_to_default_root`], and annotated with a visual marker through
//!    [`annotate_orphaned_branches`].
//! 5. **Filtering** — An optional `--include` glob narrows the graph to matching branches (plus ancestors) with
//!    [`filter_branch_graph`].
//! 6. **Rendering** — [`BranchTableRenderer`] formats the final graph as a styled, tree-aligned table written to
//!    stdout.
//!
//! All heavy lifting lives in `twig_core::git`; this module is the
//! user-facing orchestrator that wires those building blocks together with
//! CLI flags and user-friendly error messages.

use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use git2::Repository;
use twig_core::git::{
  BranchGraph, BranchGraphBuilder, BranchGraphError, BranchName, BranchTableColorMode, BranchTableRenderer,
  BranchTableSchema, BranchTableStyle, annotate_orphaned_branches, attach_orphans_to_default_root, checkout_branch,
  collect_tree_order, default_root_branch, determine_render_root, filter_branch_graph, find_orphaned_branches,
  get_repository,
};
use twig_core::output::{format_command, print_error, print_success, print_warning};
use twig_core::state::RepoState;

use crate::Cli;

/// Runs the branch-tree visualization pipeline.
///
/// This is the primary entry point for `twig-flow` when invoked without a
/// `--target` flag. It executes the full pipeline described in the
/// [module documentation](self):
///
/// 1. Locate the Git repository via [`get_repository`].
/// 2. Load [`RepoState`] to obtain root branches and dependency edges.
/// 3. Optionally switch to the root (`--root`) or parent (`--parent`) branch.
/// 4. Build a [`BranchGraph`] through [`BranchGraphBuilder`].
/// 5. Detect, attach, and annotate orphaned branches.
/// 6. Apply an `--include` filter if provided.
/// 7. Determine the render root and print the tree table.
///
/// Each step produces user-facing diagnostics (via [`twig_core::output`])
/// rather than propagating raw errors, so the function returns `Ok(())`
/// even for expected failure paths like missing repos or empty graphs.
///
/// # Errors
///
/// Returns an error only on unrecoverable problems such as failing to read
/// `.twig/state.json` when the file exists but is corrupt, or I/O errors
/// during rendering.
pub fn run(cli: &Cli) -> Result<()> {
  let repo = match get_repository() {
    Some(repo) => repo,
    None => {
      print_error("Not in a git repository. Run this command from within a repository.");
      return Ok(());
    }
  };

  let repo_state = load_repo_state(&repo)?;

  if repo_state.get_root_branches().is_empty() {
    print_error(&format!(
      "No root branches configured. Add one with {}.",
      format_command("twig branch root add <branch>")
    ));
    return Ok(());
  }

  let needs_tree_nav = cli.up || cli.down;

  let selection = if cli.root {
    select_root_branch(&repo, &repo_state)?
  } else if cli.parent {
    select_parent_branch(&repo, &repo_state)?
  } else if needs_tree_nav {
    select_tree_neighbor(&repo, &repo_state, cli.up)?
  } else {
    Selection::default()
  };

  if let Some(message) = selection.message {
    print_success(&message);
  }

  let graph = match BranchGraphBuilder::new().build(&repo) {
    Ok(graph) => graph,
    Err(err) => {
      handle_graph_error(err);
      return Ok(());
    }
  };

  if graph.is_empty() {
    print_warning("No branches found to render.");
    return Ok(());
  }

  let orphaned = find_orphaned_branches(&graph, &repo_state);

  let graph = attach_orphans_to_default_root(graph, &repo_state);
  let graph = annotate_orphaned_branches(graph, &orphaned);
  let mut highlighted = BTreeSet::new();

  let graph = if let Some(pattern) = cli.include.as_deref() {
    match filter_branch_graph(&graph, pattern) {
      Some((filtered, matches)) => {
        highlighted = matches;
        filtered
      }
      None => {
        print_warning(&format!("No branches matched pattern \"{pattern}\"."));
        return Ok(());
      }
    }
  } else {
    graph
  };

  let root = match determine_render_root(&graph, &repo_state, selection.render_root) {
    Some(root) => root,
    None => {
      print_warning("Unable to determine a branch to render.");
      return Ok(());
    }
  };

  render_table(&graph, &root, &highlighted)?;
  display_orphan_note(&orphaned);

  Ok(())
}

/// Loads the per-repository Twig state, falling back to an empty default.
///
/// Resolves the repository working directory and delegates to
/// [`RepoState::load`] which reads `.twig/state.json`. If the file is
/// missing or unreadable, a warning is printed and an empty [`RepoState`]
/// is returned so that the tree can still render (albeit without dependency
/// information or root-branch configuration).
///
/// # Errors
///
/// Returns an error only when the repository has no working directory
/// (i.e. it is a bare repo).
fn load_repo_state(repo: &Repository) -> Result<RepoState> {
  let workdir = repo
    .workdir()
    .ok_or_else(|| anyhow!("Cannot determine repository working directory"))?;

  match RepoState::load(workdir) {
    Ok(state) => Ok(state),
    Err(_) => {
      print_warning("Failed to load .twig/state.json; proceeding with empty state.");
      Ok(RepoState::default())
    }
  }
}

/// Captures the result of an optional branch-switch operation.
///
/// When the user passes `--root` or `--parent`, the plugin checks out a
/// different branch before rendering the tree. `Selection` carries the
/// outcome back to [`run`]:
///
/// * `render_root` — if set, overrides [`determine_render_root`]'s default heuristic so the tree is rooted at the
///   branch the user switched to.
/// * `message` — an optional success message to display after the checkout.
///
/// A default (empty) `Selection` means no branch switch was requested and
/// the render root will be chosen automatically by core.
#[derive(Default)]
struct Selection {
  render_root: Option<String>,
  message: Option<String>,
}

/// Checks out the default root branch and returns a [`Selection`] pinned to it.
///
/// Uses [`default_root_branch`] to resolve the configured root from
/// [`RepoState`], then delegates to [`checkout_branch`] for the actual
/// `HEAD` update. If no root is configured, an error message is printed
/// and an empty selection is returned.
///
/// # Errors
///
/// Propagates errors from [`checkout_branch`] (e.g. the branch ref is
/// missing or the working directory cannot be updated).
fn select_root_branch(repo: &Repository, state: &RepoState) -> Result<Selection> {
  if let Some(root_branch) = default_root_branch(state) {
    checkout_branch(repo, &root_branch).with_context(|| format!("Failed to checkout {root_branch}"))?;
    Ok(Selection {
      render_root: Some(root_branch.clone()),
      message: Some(format!("Switched to branch \"{root_branch}\" (root)")),
    })
  } else {
    print_error(&format!(
      "No root branches configured. Add one with {}.",
      format_command("twig branch root add <branch>")
    ));
    Ok(Selection::default())
  }
}

/// Checks out the parent of the current branch and returns a [`Selection`] pinned to it.
///
/// Resolves the current branch name from `HEAD`, then queries
/// [`RepoState::get_dependency_parents`] for its declared parent(s).
/// The function handles three edge cases with user-facing messages:
///
/// * **Detached HEAD** — cannot determine the current branch.
/// * **No parents** — the branch has no configured dependencies.
/// * **Multiple parents** — ambiguous; the user must refine dependencies.
///
/// When exactly one parent exists, [`checkout_branch`] switches to it.
///
/// # Errors
///
/// Propagates errors from [`checkout_branch`].
fn select_parent_branch(repo: &Repository, state: &RepoState) -> Result<Selection> {
  let Some(current_branch) = current_branch_name(repo) else {
    print_warning("Repository is in a detached HEAD state; cannot determine parent branch.");
    return Ok(Selection::default());
  };

  let parents = state.get_dependency_parents(&current_branch);

  if parents.is_empty() {
    print_warning("No parent branch configured for the current branch.");
    return Ok(Selection::default());
  }

  if parents.len() > 1 {
    let options = parents.join(", ");
    print_error(&format!(
      "Multiple parents configured for {current_branch}: {options}. Refine dependencies before using --parent."
    ));
    return Ok(Selection::default());
  }

  let parent = parents[0].to_string();
  checkout_branch(repo, &parent).with_context(|| format!("Failed to checkout {parent}"))?;

  Ok(Selection {
    render_root: Some(parent.clone()),
    message: Some(format!("Switched to parent branch \"{parent}\"")),
  })
}

/// Navigates to the previous (`up = true`) or next (`up = false`) branch in
/// tree-display order.
///
/// Builds a temporary [`BranchGraph`] and flattens it into depth-first order
/// (matching the rendered tree). The current branch's position is located and
/// the adjacent entry is checked out.
///
/// # Errors
///
/// Propagates errors from graph building or [`checkout_branch`].
fn select_tree_neighbor(repo: &Repository, state: &RepoState, up: bool) -> Result<Selection> {
  let Some(current) = current_branch_name(repo) else {
    print_warning("Repository is in a detached HEAD state; cannot navigate the tree.");
    return Ok(Selection::default());
  };

  let graph = match BranchGraphBuilder::new().build(repo) {
    Ok(g) => g,
    Err(err) => {
      handle_graph_error(err);
      return Ok(Selection::default());
    }
  };

  if graph.is_empty() {
    print_warning("No branches found to navigate.");
    return Ok(Selection::default());
  }

  // Apply the same orphan handling as the render pipeline so the order matches.
  let graph = attach_orphans_to_default_root(graph, state);

  let Some(root) = determine_render_root(&graph, state, None) else {
    print_warning("Unable to determine a branch to render.");
    return Ok(Selection::default());
  };

  let order = collect_tree_order(&graph, &root);
  let current_name = BranchName::from(current.as_str());

  let Some(pos) = order.iter().position(|b| b == &current_name) else {
    print_warning("Current branch is not part of the rendered tree.");
    return Ok(Selection::default());
  };

  let direction = if up { "up" } else { "down" };

  let target_pos = if up {
    if pos == 0 {
      print_warning("Already at the top of the tree.");
      return Ok(Selection::default());
    }
    pos - 1
  } else {
    if pos + 1 >= order.len() {
      print_warning("Already at the bottom of the tree.");
      return Ok(Selection::default());
    }
    pos + 1
  };

  let target = &order[target_pos];
  checkout_branch(repo, target.as_str())
    .with_context(|| format!("Failed to checkout {}", target.as_str()))?;

  Ok(Selection {
    render_root: None,
    message: Some(format!(
      "Switched {direction} to branch \"{}\"",
      target.as_str()
    )),
  })
}

/// Returns the short name of the branch `HEAD` points to, or `None` for a
/// detached `HEAD`.
fn current_branch_name(repo: &Repository) -> Option<String> {
  let head = repo.head().ok()?;
  head.shorthand().map(|s| s.to_string())
}

/// Renders the branch graph as a tree-aligned table to stdout.
///
/// Configures the core rendering pipeline:
///
/// * [`BranchTableSchema`] — uses `"—"` as the placeholder for empty cells.
/// * [`BranchTableStyle`] — color mode resolved from the `TWIG_COLORS` environment variable (see
///   [`resolve_color_mode`]).
/// * [`BranchTableRenderer`] — receives the schema, style, and any `highlighted` branches (those matched by an
///   `--include` filter) so they are visually distinguished in the output.
///
/// # Errors
///
/// Propagates formatting errors from [`BranchTableRenderer::render`].
fn render_table(graph: &BranchGraph, root: &BranchName, highlighted: &BTreeSet<BranchName>) -> Result<()> {
  let schema = BranchTableSchema::default().with_placeholder("—");
  let style = BranchTableStyle::new(resolve_color_mode());
  let mut buffer = String::new();
  BranchTableRenderer::new(schema)
    .with_style(style)
    .with_highlighted_branches(highlighted.iter().cloned())
    .render(&mut buffer, graph, root)?;
  print!("{buffer}");
  Ok(())
}

/// Maps the `TWIG_COLORS` environment variable to a [`BranchTableColorMode`].
///
/// * `"yes"` → [`BranchTableColorMode::Always`]
/// * `"no"`  → [`BranchTableColorMode::Never`]
/// * Anything else (including unset) → [`BranchTableColorMode::Auto`], which lets the renderer decide based on terminal
///   capability.
fn resolve_color_mode() -> BranchTableColorMode {
  match std::env::var("TWIG_COLORS").as_deref() {
    Ok("yes") => BranchTableColorMode::Always,
    Ok("no") => BranchTableColorMode::Never,
    _ => BranchTableColorMode::Auto,
  }
}

/// Translates a [`BranchGraphError`] into a user-friendly error message.
///
/// Each variant maps to a specific diagnostic:
///
/// * [`BranchGraphError::MissingWorkdir`] — bare repositories are unsupported.
/// * [`BranchGraphError::MissingHead`] — the repo needs at least one commit.
/// * [`BranchGraphError::Git`] — low-level `git2` failure.
/// * [`BranchGraphError::Other`] — catch-all for unexpected builder errors.
fn handle_graph_error(err: BranchGraphError) {
  match err {
    BranchGraphError::MissingWorkdir => {
      print_error("Cannot render branches for a bare repository.");
    }
    BranchGraphError::MissingHead => {
      print_error("Repository does not have a valid HEAD. Commit at least once before rendering.");
    }
    BranchGraphError::Git(inner) => {
      print_error(&format!("Failed to inspect repository: {inner}"));
    }
    BranchGraphError::Other(inner) => {
      print_error(&format!("Failed to build branch graph: {inner}"));
    }
  }
}

/// Prints a footer note when orphaned branches are present in the tree.
///
/// Orphaned branches — those with no configured dependency parents and not
/// designated as roots — are annotated with a `†` marker by
/// [`annotate_orphaned_branches`]. This function displays a legend
/// explaining the marker and directs the user to `twig adopt` for
/// re-parenting.
fn display_orphan_note(orphaned: &BTreeSet<BranchName>) {
  if orphaned.is_empty() {
    return;
  }

  println!();
  print_warning(&format!(
    "† indicates an orphaned branch (no dependencies defined). Re-parent with {}.",
    format_command("twig adopt")
  ));
}
