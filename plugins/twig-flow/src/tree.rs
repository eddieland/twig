use std::collections::BTreeSet;

use anyhow::Result;
use twig_core::git::{
  BranchGraphBuilder, BranchSelection, annotate_orphaned_branches, attach_orphans_to_default_root,
  build_branch_table_links, determine_render_root, display_orphan_note, filter_branch_graph, find_orphaned_branches,
  get_repository, handle_graph_error, load_repo_state, render_branch_table, select_parent_branch, select_root_branch,
};
use twig_core::output::{print_error, print_success, print_warning};

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
    BranchSelection::default()
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

  let links = build_branch_table_links(&repo, cli.no_links);

  render_branch_table(&graph, &root, &highlighted, links)?;
  display_orphan_note(&orphaned);

  Ok(())
}
