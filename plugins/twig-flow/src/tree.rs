use anyhow::{Context, Result, anyhow};
use git2::Repository;
use twig_core::git::{
  BranchGraph, BranchGraphBuilder, BranchGraphError, BranchName, BranchTableColorMode, BranchTableRenderer,
  BranchTableSchema, BranchTableStyle, checkout_branch, get_repository,
};
use twig_core::output::{print_error, print_success, print_warning};
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

  let graph = match BranchGraphBuilder::new().with_orphan_parenting(true).build(&repo) {
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

  let root = match determine_render_root(&graph, &repo_state, selection.render_root) {
    Some(root) => root,
    None => {
      print_warning("Unable to determine a branch to render.");
      return Ok(());
    }
  };

  render_table(&graph, &root)?;

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
    print_warning("No root branches configured; staying on the current branch.");
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

fn default_root_branch(state: &RepoState) -> Option<String> {
  state
    .get_default_root()
    .map(|root| root.to_string())
    .or_else(|| state.get_root_branches().first().cloned())
}

fn current_branch_name(repo: &Repository) -> Option<String> {
  let head = repo.head().ok()?;
  head.shorthand().map(|s| s.to_string())
}

fn determine_render_root(
  graph: &BranchGraph,
  state: &RepoState,
  override_branch: Option<String>,
) -> Option<BranchName> {
  if let Some(branch) = override_branch {
    let target = BranchName::from(branch.clone());
    if graph.get(&target).is_some() {
      return Some(target);
    }
  }

  if let Some(root) = state.get_default_root() {
    let candidate = BranchName::from(root.to_string());
    if graph.get(&candidate).is_some() {
      return Some(candidate);
    }
  }

  if let Some(candidate) = graph.root_candidates().first() {
    return Some(candidate.clone());
  }

  if let Some(branch) = graph.current_branch() {
    return Some(branch.clone());
  }

  graph.iter().next().map(|(_, node)| node.name.clone())
}

fn render_table(graph: &BranchGraph, root: &BranchName) -> Result<()> {
  let schema = BranchTableSchema::default().with_placeholder("—");
  let style = BranchTableStyle::new(resolve_color_mode());
  let mut buffer = String::new();
  BranchTableRenderer::new(schema)
    .with_style(style)
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
