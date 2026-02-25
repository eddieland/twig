//! # Tidy Command
//!
//! Implementation of the tidy command for cleaning up branches and maintaining
//! the twig tree structure.

use std::collections::HashSet;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use git2::Repository as Git2Repository;
use twig_core::detect_repository;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::state::RepoState;

/// Command for cleaning up branches and twig tree
#[derive(Args)]
pub struct TidyArgs {
  #[command(subcommand)]
  pub command: Option<TidyCommand>,

  /// Show which branches would be deleted without actually deleting them
  #[arg(long = "dry-run")]
  pub dry_run: bool,

  /// Skip confirmation prompt and delete branches immediately
  #[arg(short = 'f', long = "force")]
  pub force: bool,
}

/// Subcommands for the tidy command
#[derive(Subcommand)]
pub enum TidyCommand {
  /// Clean up branches with no unique commits and no children (default
  /// behavior)
  Clean(CleanArgs),

  /// Remove deleted branches from the twig tree configuration
  Prune(PruneArgs),
}

/// Arguments for the clean subcommand
#[derive(Args)]
pub struct CleanArgs {
  /// Show which branches would be deleted without actually deleting them
  #[arg(long = "dry-run")]
  pub dry_run: bool,

  /// Skip confirmation prompt and delete branches immediately
  #[arg(short = 'f', long = "force")]
  pub force: bool,

  /// Aggressively clean up by reparenting branches when intermediate branches
  /// have no changes
  #[arg(short = 'a', long = "aggressive")]
  pub aggressive: bool,
}

/// Arguments for the prune subcommand
#[derive(Args)]
pub struct PruneArgs {
  /// Show which branches would be removed from configuration without
  /// actually removing them
  #[arg(long = "dry-run")]
  pub dry_run: bool,

  /// Skip confirmation prompt and remove references immediately
  #[arg(short = 'f', long = "force")]
  pub force: bool,
}

/// Handle the tidy command
pub(crate) fn handle_tidy_command(tidy: TidyArgs) -> Result<()> {
  match tidy.command {
    Some(TidyCommand::Clean(args)) => handle_clean_command(args),
    Some(TidyCommand::Prune(args)) => handle_prune_command(args),
    None => {
      // Backward compatibility: if no subcommand is provided, run clean
      let clean_args = CleanArgs {
        dry_run: tidy.dry_run,
        force: tidy.force,
        aggressive: false,
      };
      handle_clean_command(clean_args)
    }
  }
}

/// Handle the clean subcommand
///
/// Finds branches that have no unique commits compared to their parent and have
/// no child branches, then deletes them.
fn handle_clean_command(clean: CleanArgs) -> Result<()> {
  let repo_path = detect_repository().context("Not in a git repository")?;
  let repo = Git2Repository::open(&repo_path)?;
  let repo_state = RepoState::load(&repo_path)?;

  let branches = repo
    .branches(Some(git2::BranchType::Local))?
    .collect::<Result<Vec<_>, _>>()
    .context("Failed to collect branches")?;

  let mut branches_to_delete = Vec::new();
  let mut reparenting_operations = Vec::new();
  let mut processed_chains = HashSet::new();

  print_info("Analyzing branches for cleanup...");

  // If aggressive mode is enabled, first handle reparenting
  if clean.aggressive {
    reparenting_operations = find_reparenting_opportunities(&repo_state, &repo)?;

    if !reparenting_operations.is_empty() {
      print_info(&format!("Found {} reparenting opportunities:", reparenting_operations.len()));
      for (child, old_parent, new_parent) in &reparenting_operations {
        print_info(&format!("  • {} will be reparented from {} to {}", child, old_parent, new_parent));
      }
    }
  }

  for (branch, _) in branches {
    let branch_name = match branch.name()? {
      Some(name) => name.to_string(),
      None => continue,
    };

    if is_current_branch(&repo, &branch_name)? {
      continue;
    }

    if processed_chains.contains(&branch_name) {
      continue;
    }

    if has_non_cleanable_children(&repo_state, &repo, &branch_name)? {
      continue;
    }

    let parent_branch = find_parent_branch(&repo_state, &repo, &branch_name)?;

    if let Some(parent) = parent_branch {
      if !has_unique_commits(&repo, &branch_name, &parent)? {
        let chain = find_cleanable_dependency_chain(&repo_state, &repo, &branch_name)?;

        for chain_branch in &chain {
          processed_chains.insert(chain_branch.clone());
          if !branches_to_delete.contains(chain_branch) {
            branches_to_delete.push(chain_branch.clone());
          }
        }

        if chain.is_empty() {
          branches_to_delete.push(branch_name);
        }
      }
    }
  }

  if branches_to_delete.is_empty() && reparenting_operations.is_empty() {
    print_info("No branches found that can be tidied up.");
    return Ok(());
  }

  if !branches_to_delete.is_empty() {
    print_info(&format!("Found {} branches to delete:", branches_to_delete.len()));
    for branch in &branches_to_delete {
      print_info(&format!("  • {}", branch));
    }
  }

  if !reparenting_operations.is_empty() {
    print_info(&format!("Found {} reparenting operations:", reparenting_operations.len()));
    for (child, old_parent, new_parent) in &reparenting_operations {
      print_info(&format!("  • {} will be reparented from {} to {}", child, old_parent, new_parent));
    }
  }

  if clean.dry_run {
    print_info("Dry run mode - no changes were actually made.");
    return Ok(());
  }

  // Confirm operations unless --force is used
  if !clean.force {
    if !branches_to_delete.is_empty() {
      print_warning("This will permanently delete the listed branches.");
    } else {
      print_warning("This will reparent the listed branches.");
    }
    print_info("Use --force to skip this confirmation, or --dry-run to preview.");
    // In non-interactive mode, prompt for confirmation
    use std::io::{self, Write};
    #[allow(clippy::print_stdout)]
    {
      print!("Continue? (y/N): ");
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().to_lowercase().starts_with('y') {
      print_info("Operation cancelled.");
      return Ok(());
    }
  }

  // Perform reparenting operations first
  let mut repo_state = repo_state;
  let mut reparented_count = 0;

  for (child, old_parent, new_parent) in reparenting_operations {
    if repo_state.remove_dependency(&child, &old_parent) {
      match repo_state.add_dependency(child.clone(), new_parent.clone()) {
        Ok(()) => {
          print_success(&format!("Reparented {} from {} to {}", child, old_parent, new_parent));
          reparented_count += 1;

          // If the old parent now has no children and no unique commits, mark for deletion
          if repo_state.get_dependency_children(&old_parent).is_empty()
            && let Ok(parent_of_old) = find_parent_branch(&repo_state, &repo, &old_parent)
            && let Some(parent) = parent_of_old
            && let Ok(false) = has_unique_commits(&repo, &old_parent, &parent)
            && !branches_to_delete.contains(&old_parent)
          {
            branches_to_delete.push(old_parent.clone());
            print_info(&format!("Added {} to deletion list (no children, no changes)", old_parent));
          }
        }
        Err(e) => {
          print_error(&format!("Failed to reparent {} from {} to {}: {}", child, old_parent, new_parent, e));
          // Try to restore the old dependency
          let _ = repo_state.add_dependency(child, old_parent);
        }
      }
    }
  }

  // Perform deletion operations
  let mut deleted_count = 0;

  for branch_name in branches_to_delete {
    match delete_branch(&repo, &branch_name) {
      Ok(()) => {
        cleanup_branch_from_config(&mut repo_state, &branch_name);
        print_success(&format!("Deleted branch: {}", branch_name));
        deleted_count += 1;
      }
      Err(e) => {
        print_error(&format!("Failed to delete branch {}: {}", branch_name, e));
      }
    }
  }

  // Save the updated configuration
  if deleted_count > 0 || reparented_count > 0 {
    if let Err(e) = repo_state.save(&repo_path) {
      print_warning(&format!("Failed to save updated configuration: {}", e));
    } else {
      print_info("Updated twig configuration.");
    }
  }

  if deleted_count > 0 {
    print_success(&format!("Clean complete: deleted {} branches.", deleted_count));
  }
  if reparented_count > 0 {
    print_success(&format!("Clean complete: reparented {} branches.", reparented_count));
  }

  Ok(())
}

/// Handle the prune subcommand
///
/// Finds branches that are referenced in the twig configuration but no longer
/// exist in the Git repository, then removes the stale references.
fn handle_prune_command(prune: PruneArgs) -> Result<()> {
  let repo_path = detect_repository().context("Not in a git repository")?;
  let repo = Git2Repository::open(&repo_path)?;
  let mut repo_state = RepoState::load(&repo_path)?;

  let existing_branches: HashSet<String> = repo
    .branches(Some(git2::BranchType::Local))?
    .filter_map(|branch_result| {
      branch_result
        .ok()
        .and_then(|(branch, _)| branch.name().ok().flatten().map(|name| name.to_string()))
    })
    .collect();

  print_info("Analyzing twig configuration for deleted branches...");

  let mut branches_to_remove = Vec::new();

  // Check dependencies for non-existent branches
  for dependency in &repo_state.dependencies {
    if !existing_branches.contains(&dependency.child) {
      branches_to_remove.push(dependency.child.clone());
    }
    if !existing_branches.contains(&dependency.parent) {
      branches_to_remove.push(dependency.parent.clone());
    }
  }

  // Check root branches for non-existent branches
  for root_branch in &repo_state.root_branches {
    if !existing_branches.contains(&root_branch.branch) {
      branches_to_remove.push(root_branch.branch.clone());
    }
  }

  // Check metadata for non-existent branches
  for branch_name in repo_state.branches.keys() {
    if !existing_branches.contains(branch_name) {
      branches_to_remove.push(branch_name.clone());
    }
  }

  // Remove duplicates
  branches_to_remove.sort();
  branches_to_remove.dedup();

  if branches_to_remove.is_empty() {
    print_info("No stale branch references found in twig configuration.");
    return Ok(());
  }

  print_info(&format!(
    "Found {} deleted branches to remove from twig configuration:",
    branches_to_remove.len()
  ));
  for branch in &branches_to_remove {
    print_info(&format!("  • {}", branch));
  }

  if prune.dry_run {
    print_info("Dry run mode - no configuration changes were made.");
    return Ok(());
  }

  if !prune.force {
    print_warning("This will remove the stale references from twig configuration.");
    use std::io::{self, Write};
    #[allow(clippy::print_stdout)]
    {
      print!("Continue? (y/N): ");
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().to_lowercase().starts_with('y') {
      print_info("Operation cancelled.");
      return Ok(());
    }
  }

  let mut removed_count = 0;

  for branch in &branches_to_remove {
    removed_count += repo_state.remove_all_dependencies_for_branch(branch);
    if repo_state.remove_root(branch) {
      removed_count += 1;
    }
    if repo_state.branches.remove(branch).is_some() {
      removed_count += 1;
    }
  }

  if removed_count > 0 {
    match repo_state.save(&repo_path) {
      Ok(()) => {
        print_success(&format!(
          "Prune complete: removed {} stale references from twig configuration.",
          removed_count
        ));
      }
      Err(e) => {
        print_error(&format!("Failed to save updated configuration: {}", e));
        return Err(e);
      }
    }
  }

  Ok(())
}

/// Check if a branch is the current (HEAD) branch
fn is_current_branch(repo: &Git2Repository, branch_name: &str) -> Result<bool> {
  let head = repo.head()?;
  if let Some(current) = head.shorthand() {
    Ok(current == branch_name)
  } else {
    Ok(false)
  }
}

/// Check if a branch has children that are not part of a cleanable chain
fn has_non_cleanable_children(repo_state: &RepoState, repo: &Git2Repository, branch_name: &str) -> Result<bool> {
  let children = repo_state.get_dependency_children(branch_name);

  for child in children {
    if is_current_branch(repo, child)? {
      return Ok(true);
    }

    if has_unique_commits(repo, child, branch_name)? {
      return Ok(true);
    }

    if has_non_cleanable_children(repo_state, repo, child)? {
      return Ok(true);
    }
  }

  Ok(false)
}

/// Find a cleanable dependency chain starting from a branch
fn find_cleanable_dependency_chain(
  repo_state: &RepoState,
  repo: &Git2Repository,
  start_branch: &str,
) -> Result<Vec<String>> {
  let mut chain = Vec::new();
  let mut current = start_branch;

  loop {
    let children = repo_state.get_dependency_children(current);

    if children.is_empty() {
      chain.push(current.to_string());
      break;
    }

    if children.len() > 1 {
      break;
    }

    let child = children[0];

    if is_current_branch(repo, child)? {
      break;
    }

    if has_unique_commits(repo, child, current)? {
      break;
    }

    chain.push(current.to_string());
    current = child;
  }

  if chain.len() > 1 || repo_state.get_dependency_children(start_branch).is_empty() {
    Ok(chain)
  } else {
    Ok(Vec::new())
  }
}

/// Find reparenting opportunities for aggressive cleanup
fn find_reparenting_opportunities(
  repo_state: &RepoState,
  repo: &Git2Repository,
) -> Result<Vec<(String, String, String)>> {
  let mut reparenting_ops = Vec::new();

  for dependency in &repo_state.dependencies {
    let intermediate_branch = &dependency.parent;
    let child_branch = &dependency.child;

    if is_current_branch(repo, intermediate_branch)? {
      continue;
    }

    let grandparent = find_parent_branch(repo_state, repo, intermediate_branch)?;

    if let Some(grandparent_name) = grandparent {
      if !has_unique_commits(repo, intermediate_branch, &grandparent_name)? {
        let children = repo_state.get_dependency_children(intermediate_branch);
        if children.len() == 1 && children[0] == child_branch {
          reparenting_ops.push((child_branch.clone(), intermediate_branch.clone(), grandparent_name));
        }
      }
    }
  }

  Ok(reparenting_ops)
}

/// Find the parent branch for a given branch
fn find_parent_branch(repo_state: &RepoState, repo: &Git2Repository, branch_name: &str) -> Result<Option<String>> {
  let parents = repo_state.get_dependency_parents(branch_name);
  if let Some(parent) = parents.first() {
    return Ok(Some(parent.to_string()));
  }

  // Fall back to trying common parent branches
  let potential_parents = ["main", "master", "develop"];
  for parent in potential_parents {
    if repo.find_branch(parent, git2::BranchType::Local).is_ok() {
      return Ok(Some(parent.to_string()));
    }
  }

  Ok(None)
}

/// Check if a branch has unique commits compared to its parent
fn has_unique_commits(repo: &Git2Repository, branch_name: &str, parent_name: &str) -> Result<bool> {
  let branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
  let parent = repo.find_branch(parent_name, git2::BranchType::Local)?;

  let branch_commit = branch
    .get()
    .target()
    .ok_or_else(|| anyhow::anyhow!("Branch has no target commit"))?;
  let parent_commit = parent
    .get()
    .target()
    .ok_or_else(|| anyhow::anyhow!("Parent branch has no target commit"))?;

  if branch_commit == parent_commit {
    return Ok(false);
  }

  let mut revwalk = repo.revwalk()?;
  revwalk.push(branch_commit)?;
  revwalk.hide(parent_commit)?;

  Ok(revwalk.next().is_some())
}

/// Delete a branch
fn delete_branch(repo: &Git2Repository, branch_name: &str) -> Result<()> {
  let mut branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
  branch
    .delete()
    .with_context(|| format!("Failed to delete branch '{}'", branch_name))?;
  Ok(())
}

/// Clean up a branch from the twig configuration
fn cleanup_branch_from_config(repo_state: &mut RepoState, branch_name: &str) {
  let removed_dependencies = repo_state.remove_all_dependencies_for_branch(branch_name);
  let removed_from_roots = repo_state.remove_root(branch_name);

  if removed_dependencies > 0 || removed_from_roots {
    print_info(&format!(
      "Cleaned up twig config for '{}': {} dependencies, {} root entries removed",
      branch_name,
      removed_dependencies,
      if removed_from_roots { 1 } else { 0 }
    ));
  }
}

#[cfg(test)]
mod tests {
  use tempfile::TempDir;
  use twig_core::state::RepoState;

  use super::*;

  fn create_mock_repo_state() -> RepoState {
    let mut state = RepoState::default();

    state
      .add_dependency("branch-b".to_string(), "branch-a".to_string())
      .expect("add dep");
    state
      .add_dependency("branch-c".to_string(), "branch-b".to_string())
      .expect("add dep");

    state
  }

  #[test]
  fn test_has_non_cleanable_children_empty() {
    let repo_state = RepoState::default();
    let temp_dir = TempDir::new().expect("temp dir");
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).expect("init bare");

    let result = has_non_cleanable_children(&repo_state, &mock_repo, "nonexistent");
    assert!(result.is_ok());
    assert!(!result.expect("result"));
  }

  #[test]
  fn test_find_cleanable_dependency_chain_empty() {
    let repo_state = RepoState::default();
    let temp_dir = TempDir::new().expect("temp dir");
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).expect("init bare");

    let result = find_cleanable_dependency_chain(&repo_state, &mock_repo, "nonexistent");
    assert!(result.is_ok());
    let chain = result.expect("chain");

    // A nonexistent branch with no children should return itself as a leaf
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], "nonexistent");
  }

  #[test]
  fn test_find_cleanable_dependency_chain_single_branch() {
    let repo_state = create_mock_repo_state();
    let temp_dir = TempDir::new().expect("temp dir");
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).expect("init bare");

    let result = find_cleanable_dependency_chain(&repo_state, &mock_repo, "branch-c");
    assert!(result.is_ok());
    let chain = result.expect("chain");

    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], "branch-c");
  }

  #[test]
  fn test_find_reparenting_opportunities_empty_state() {
    let repo_state = RepoState::default();
    let temp_dir = TempDir::new().expect("temp dir");
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).expect("init bare");

    let result = find_reparenting_opportunities(&repo_state, &mock_repo);
    assert!(result.is_ok());
    assert!(result.expect("ops").is_empty());
  }
}
