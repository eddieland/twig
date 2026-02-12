use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use git2::Repository;
use twig_core::git::{
  BranchGraph, BranchGraphBuilder, BranchGraphError, BranchName, BranchTableColorMode, BranchTableRenderer,
  BranchTableSchema, BranchTableStyle, annotate_orphaned_branches, attach_orphans_to_default_root, checkout_branch,
  default_root_branch, determine_render_root, filter_branch_graph, find_orphaned_branches, get_repository,
};
use twig_core::output::{format_command, print_error, print_success, print_warning};
use twig_core::state::RepoState;

use crate::Cli;

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

  let selection = if cli.root {
    select_root_branch(&repo, &repo_state)?
  } else if cli.parent {
    select_parent_branch(&repo, &repo_state)?
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

#[derive(Default)]
struct Selection {
  render_root: Option<String>,
  message: Option<String>,
}

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

fn current_branch_name(repo: &Repository) -> Option<String> {
  let head = repo.head().ok()?;
  head.shorthand().map(|s| s.to_string())
}

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

fn resolve_color_mode() -> BranchTableColorMode {
  match std::env::var("TWIG_COLORS").as_deref() {
    Ok("yes") => BranchTableColorMode::Always,
    Ok("no") => BranchTableColorMode::Never,
    _ => BranchTableColorMode::Auto,
  }
}

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
