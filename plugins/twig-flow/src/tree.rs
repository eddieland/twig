use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::{Context, Result, anyhow};
use git2::Repository;
use twig_core::git::{
  BranchAnnotationValue, BranchEdge, BranchGraph, BranchGraphBuilder, BranchGraphError, BranchName, BranchNode,
  BranchTableColorMode, BranchTableLinkMode, BranchTableLinks, BranchTableRenderer, BranchTableSchema,
  BranchTableStyle, ORPHAN_BRANCH_ANNOTATION_KEY, checkout_branch, extract_github_repo_from_url, get_repository,
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

  let links = build_links(&repo, cli.no_links);

  render_table(&graph, &root, &highlighted, links)?;
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

fn render_table(
  graph: &BranchGraph,
  root: &BranchName,
  highlighted: &BTreeSet<BranchName>,
  links: BranchTableLinks,
) -> Result<()> {
  let schema = BranchTableSchema::default().with_placeholder("—");
  let style = BranchTableStyle::new(resolve_color_mode());
  let mut buffer = String::new();
  BranchTableRenderer::new(schema)
    .with_style(style)
    .with_links(links)
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

fn build_links(repo: &Repository, disable_links: bool) -> BranchTableLinks {
  let mut links = BranchTableLinks::new(if disable_links {
    BranchTableLinkMode::Never
  } else {
    BranchTableLinkMode::Auto
  });

  if let Some(jira_base) = resolve_jira_base_url() {
    links = links.with_jira_base_url(jira_base);
  }

  if let Some((owner, repo_name)) = resolve_github_repo(repo) {
    links = links.with_github_repo(owner, repo_name);
  }

  links
}

fn resolve_jira_base_url() -> Option<String> {
  let host = std::env::var("JIRA_HOST").ok()?;
  let trimmed = host.trim_end_matches('/');
  if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
    Some(trimmed.to_string())
  } else {
    Some(format!("https://{trimmed}"))
  }
}

fn resolve_github_repo(repo: &Repository) -> Option<(String, String)> {
  let remote = repo.find_remote("origin").ok()?;
  let url = remote.url()?;
  extract_github_repo_from_url(url).ok()
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

fn attach_orphans_to_default_root(graph: BranchGraph, repo_state: &RepoState) -> BranchGraph {
  let Some(default_root) = default_root_branch(repo_state) else {
    return graph;
  };

  let default_root_name = BranchName::from(default_root.as_str());

  let mut nodes: BTreeMap<BranchName, BranchNode> =
    graph.iter().map(|(name, node)| (name.clone(), node.clone())).collect();

  let Some(root_node_name) = nodes.get(&default_root_name).map(|node| node.name.clone()) else {
    return graph;
  };

  let configured_roots: BTreeSet<_> = repo_state.get_root_branches().into_iter().collect();
  let orphan_names: Vec<BranchName> = nodes
    .iter()
    .filter_map(|(name, node)| {
      if node.topology.primary_parent.is_none()
        && name != &default_root_name
        && !configured_roots.contains(name.as_str())
      {
        Some(name.clone())
      } else {
        None
      }
    })
    .collect();

  if orphan_names.is_empty() {
    return graph;
  }

  let mut edges = graph.edges().to_vec();
  let root_candidates = graph.root_candidates().to_vec();
  let current_branch = graph.current_branch().cloned();

  let mut child_names = Vec::new();
  for orphan_name in &orphan_names {
    if let Some(orphan_node) = nodes.get_mut(orphan_name) {
      orphan_node.topology.primary_parent = Some(root_node_name.clone());
      child_names.push(orphan_node.name.clone());
    }
  }

  if let Some(root_node) = nodes.get_mut(&root_node_name) {
    for child_name in &child_names {
      if !root_node.topology.children.iter().any(|child| child == child_name) {
        root_node.topology.children.push(child_name.clone());
      }
      edges.push(BranchEdge::new(root_node_name.clone(), child_name.clone()));
    }
    root_node.topology.children.sort();
  }

  BranchGraph::from_parts(nodes.into_values(), edges, root_candidates, current_branch)
}

fn annotate_orphaned_branches(graph: BranchGraph, orphaned: &BTreeSet<BranchName>) -> BranchGraph {
  if orphaned.is_empty() {
    return graph;
  }

  let nodes: BTreeMap<BranchName, BranchNode> = graph
    .iter()
    .map(|(name, node)| {
      let mut node = node.clone();
      if orphaned.contains(name) {
        node.metadata.annotations.insert(
          ORPHAN_BRANCH_ANNOTATION_KEY.to_string(),
          BranchAnnotationValue::Flag(true),
        );
      }
      (name.clone(), node)
    })
    .collect();

  BranchGraph::from_parts(
    nodes.into_values(),
    graph.edges().to_vec(),
    graph.root_candidates().to_vec(),
    graph.current_branch().cloned(),
  )
}

fn find_orphaned_branches(graph: &BranchGraph, repo_state: &RepoState) -> BTreeSet<BranchName> {
  let configured_roots: HashSet<_> = repo_state.get_root_branches().into_iter().collect();

  let orphaned: BTreeSet<BranchName> = graph
    .iter()
    .filter_map(|(name, _)| {
      let branch = name.as_str();
      let has_parent = !repo_state.get_dependency_parents(branch).is_empty();
      if has_parent || configured_roots.contains(branch) {
        None
      } else {
        Some(name.clone())
      }
    })
    .collect();

  orphaned
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

fn filter_branch_graph(graph: &BranchGraph, pattern: &str) -> Option<(BranchGraph, BTreeSet<BranchName>)> {
  let needle = pattern.to_lowercase();
  let mut matches = BTreeSet::new();

  for (name, _) in graph.iter() {
    if name.as_str().to_lowercase().contains(&needle) {
      matches.insert(name.clone());
    }
  }

  if matches.is_empty() {
    return None;
  }

  let mut allowed = matches.clone();
  let mut stack: Vec<BranchName> = matches.iter().cloned().collect();

  while let Some(current) = stack.pop() {
    if let Some(node) = graph.get(&current)
      && let Some(parent) = node.topology.primary_parent.as_ref()
      && allowed.insert(parent.clone())
    {
      stack.push(parent.clone());
    }
  }

  let mut nodes = BTreeMap::new();
  for (name, node) in graph.iter() {
    if allowed.contains(name) {
      let mut filtered_node = node.clone();
      filtered_node.topology.children.retain(|child| allowed.contains(child));
      nodes.insert(name.clone(), filtered_node);
    }
  }

  let edges = graph
    .edges()
    .iter()
    .filter(|edge| allowed.contains(&edge.from) && allowed.contains(&edge.to))
    .cloned()
    .collect::<Vec<BranchEdge>>();

  let root_candidates = graph
    .root_candidates()
    .iter()
    .filter(|candidate| allowed.contains(candidate))
    .cloned()
    .collect::<Vec<BranchName>>();

  let current_branch = graph.current_branch().filter(|name| allowed.contains(name)).cloned();

  Some((
    BranchGraph::from_parts(nodes.into_values(), edges, root_candidates, current_branch),
    matches,
  ))
}

#[cfg(test)]
mod tests {
  use git2::Oid;
  use twig_core::git::{BranchHead, BranchKind, BranchTopology};

  use super::*;

  #[test]
  fn filter_keeps_matching_branches_and_ancestors() {
    let mut root = branch_node("main");
    let mut feature = branch_node("feature/payment");
    feature.topology.primary_parent = Some(root.name.clone());
    root.topology.children.push(feature.name.clone());

    let mut api = branch_node("feature/payment-api");
    api.topology.primary_parent = Some(feature.name.clone());
    feature.topology.children.push(api.name.clone());

    let mut ui = branch_node("feature/payment-ui");
    ui.topology.primary_parent = Some(feature.name.clone());
    feature.topology.children.push(ui.name.clone());

    let mut other = branch_node("feature/other");
    other.topology.primary_parent = Some(root.name.clone());
    root.topology.children.push(other.name.clone());

    let edges = vec![
      BranchEdge::new(root.name.clone(), feature.name.clone()),
      BranchEdge::new(feature.name.clone(), api.name.clone()),
      BranchEdge::new(feature.name.clone(), ui.name.clone()),
      BranchEdge::new(root.name.clone(), other.name.clone()),
    ];

    let graph = BranchGraph::from_parts(
      vec![root.clone(), feature.clone(), api.clone(), ui.clone(), other.clone()],
      edges,
      vec![root.name.clone()],
      Some(root.name.clone()),
    );

    let (filtered, matches) = filter_branch_graph(&graph, "api").expect("expected matches");

    assert!(matches.contains(&api.name));
    assert_eq!(matches.len(), 1);
    assert!(filtered.get(&api.name).is_some());
    assert!(filtered.get(&feature.name).is_some());
    assert!(filtered.get(&root.name).is_some());
    assert!(filtered.get(&ui.name).is_none());
    assert!(filtered.get(&other.name).is_none());

    let parent = filtered
      .get(&api.name)
      .and_then(|node| node.topology.primary_parent.as_ref())
      .expect("parent retained");
    assert_eq!(parent, &feature.name);
  }

  fn branch_node(name: &str) -> BranchNode {
    BranchNode {
      name: BranchName::from(name),
      kind: BranchKind::Local,
      head: BranchHead {
        oid: Oid::from_str("0123456789abcdef0123456789abcdef01234567").unwrap(),
        summary: Some(format!("Summary for {name}")),
        author: Some("Twig Bot".to_string()),
        committed_at: None,
      },
      upstream: None,
      topology: BranchTopology::default(),
      metadata: Default::default(),
    }
  }
}
