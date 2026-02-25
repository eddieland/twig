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
  /// Subcommand (`clean` or `prune`); defaults to `clean` if omitted
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

  /// Aggressively clean up by reparenting branches and using common root
  /// branches (main/master/develop) as fallback parents
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
/// no child branches, then deletes them. In `--aggressive` mode also reparents
/// intermediate branches before deletion.
fn handle_clean_command(clean: CleanArgs) -> Result<()> {
  let repo_path = detect_repository().context("Not in a git repository")?;
  let repo = Git2Repository::open(&repo_path)?;
  let repo_state = RepoState::load(&repo_path)?;

  let branches = repo
    .branches(Some(git2::BranchType::Local))?
    .collect::<Result<Vec<_>, _>>()
    .context("Failed to collect branches")?;

  print_info("Analyzing branches for cleanup...");

  // Use a HashSet for O(1) membership checks and a Vec for stable display
  // order (Issue #13).
  let mut branches_to_delete_set: HashSet<String> = HashSet::new();
  let mut branches_to_delete: Vec<String> = Vec::new();
  let mut processed_chains: HashSet<String> = HashSet::new();

  // ------------------------------------------------------------------
  // Pre-pass: discover all reparenting opportunities and their resulting
  // deletions *before* showing the confirmation prompt (Issue #3 / #4).
  // ------------------------------------------------------------------
  let reparenting_operations = if clean.aggressive {
    let ops = find_reparenting_opportunities(&repo_state, &repo)?;

    // Simulate reparenting: collect old_parents that would become deletable
    // after their sole child is reparented away.
    for (_, old_parent, grandparent_name) in &ops {
      if let Ok(false) = has_unique_commits(&repo, old_parent, grandparent_name)
        && branches_to_delete_set.insert(old_parent.clone())
      {
        branches_to_delete.push(old_parent.clone());
      }
    }

    ops
  } else {
    Vec::new()
  };

  // ------------------------------------------------------------------
  // Main scan: find directly cleanable branches.
  // ------------------------------------------------------------------
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

    // Issue #8: only use common-root fallback in --aggressive mode.
    let parent_branch =
      find_parent_branch(&repo_state, &repo, &branch_name, clean.aggressive)?;

    // Issue #5: collapse the two nested `if` into one (clippy::collapsible_if).
    if let Some(parent) = parent_branch
      && !has_unique_commits(&repo, &branch_name, &parent)?
    {
      // Issue #12: `None` means "branch_name alone is cleanable (not part of a
      // multi-branch chain)".  `Some(chain)` means clean all branches in the
      // chain.
      match find_cleanable_dependency_chain(&repo_state, &repo, &branch_name)? {
        Some(chain) => {
          for chain_branch in &chain {
            processed_chains.insert(chain_branch.clone());
            if branches_to_delete_set.insert(chain_branch.clone()) {
              branches_to_delete.push(chain_branch.clone());
            }
          }
        }
        None => {
          if branches_to_delete_set.insert(branch_name.clone()) {
            branches_to_delete.push(branch_name);
          }
        }
      }
    }
  }

  if branches_to_delete.is_empty() && reparenting_operations.is_empty() {
    print_info("No branches found that can be tidied up.");
    return Ok(());
  }

  if !branches_to_delete.is_empty() {
    print_info(&format!("Found {} branch(es) to delete:", branches_to_delete.len()));
    for branch in &branches_to_delete {
      print_info(&format!("  • {}", branch));
    }
  }

  if !reparenting_operations.is_empty() {
    print_info(&format!(
      "Found {} reparenting operation(s):",
      reparenting_operations.len()
    ));
    for (child, old_parent, new_parent) in &reparenting_operations {
      print_info(&format!(
        "  • {} will be reparented from {} to {}",
        child, old_parent, new_parent
      ));
    }
  }

  if clean.dry_run {
    print_info("Dry run mode - no changes were actually made.");
    return Ok(());
  }

  // Confirm all operations with the user unless --force is used.
  if !clean.force {
    if !branches_to_delete.is_empty() {
      print_warning("This will permanently delete the listed branches.");
    } else {
      print_warning("This will reparent the listed branches.");
    }
    print_info("Use --force to skip this confirmation, or --dry-run to preview.");

    if !crate::utils::prompt_for_confirmation("Continue?")? {
      print_info("Operation cancelled.");
      return Ok(());
    }
  }

  // ------------------------------------------------------------------
  // Execute reparenting operations first.
  // ------------------------------------------------------------------
  let mut repo_state = repo_state;
  let mut reparented_count = 0;

  for (child, old_parent, new_parent) in reparenting_operations {
    if repo_state.remove_dependency(&child, &old_parent) {
      match repo_state.add_dependency(child.clone(), new_parent.clone()) {
        Ok(()) => {
          print_success(&format!(
            "Reparented {} from {} to {}",
            child, old_parent, new_parent
          ));
          reparented_count += 1;
        }
        Err(e) => {
          print_error(&format!(
            "Failed to reparent {} from {} to {}: {}",
            child, old_parent, new_parent, e
          ));
          // Attempt to restore the original dependency.
          let _ = repo_state.add_dependency(child, old_parent);
        }
      }
    }
  }

  // ------------------------------------------------------------------
  // Execute deletion operations.
  // ------------------------------------------------------------------
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

  // Save the updated configuration.
  if deleted_count > 0 || reparented_count > 0 {
    if let Err(e) = repo_state.save(&repo_path) {
      print_warning(&format!("Failed to save updated configuration: {}", e));
    } else {
      print_info("Updated twig configuration.");
    }
  }

  if deleted_count > 0 {
    print_success(&format!("Clean complete: deleted {} branch(es).", deleted_count));
  }
  if reparented_count > 0 {
    print_success(&format!(
      "Clean complete: reparented {} branch(es).",
      reparented_count
    ));
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

  // Check dependencies for non-existent branches.
  for dependency in &repo_state.dependencies {
    if !existing_branches.contains(&dependency.child) {
      branches_to_remove.push(dependency.child.clone());
    }
    if !existing_branches.contains(&dependency.parent) {
      branches_to_remove.push(dependency.parent.clone());
    }
  }

  // Check root branches for non-existent branches.
  for root_branch in &repo_state.root_branches {
    if !existing_branches.contains(&root_branch.branch) {
      branches_to_remove.push(root_branch.branch.clone());
    }
  }

  // Check metadata for non-existent branches.
  for branch_name in repo_state.branches.keys() {
    if !existing_branches.contains(branch_name) {
      branches_to_remove.push(branch_name.clone());
    }
  }

  // Remove duplicates.
  branches_to_remove.sort();
  branches_to_remove.dedup();

  if branches_to_remove.is_empty() {
    print_info("No stale branch references found in twig configuration.");
    return Ok(());
  }

  print_info(&format!(
    "Found {} deleted branch(es) to remove from twig configuration:",
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

    if !crate::utils::prompt_for_confirmation("Continue?")? {
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
          "Prune complete: removed {} stale reference(s) from twig configuration.",
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

/// Check if a branch is the current (HEAD) branch.
///
/// Returns `false` when HEAD is detached or unborn instead of propagating the
/// error, so callers never abort a tidy operation because of a detached HEAD.
/// (Issue #1)
fn is_current_branch(repo: &Git2Repository, branch_name: &str) -> Result<bool> {
  match repo.head() {
    Ok(head) => Ok(head.shorthand() == Some(branch_name)),
    // Detached HEAD or unborn HEAD — branch_name is definitely not current.
    Err(_) => Ok(false),
  }
}

/// Check if a branch has children that are not part of a cleanable chain.
///
/// Uses a `visited` set to guard against dependency cycles in corrupt twig
/// state and prevent unbounded recursion. (Issue #2)
fn has_non_cleanable_children(
  repo_state: &RepoState,
  repo: &Git2Repository,
  branch_name: &str,
) -> Result<bool> {
  let mut visited = HashSet::new();
  has_non_cleanable_children_inner(repo_state, repo, branch_name, &mut visited)
}

fn has_non_cleanable_children_inner(
  repo_state: &RepoState,
  repo: &Git2Repository,
  branch_name: &str,
  visited: &mut HashSet<String>,
) -> Result<bool> {
  if !visited.insert(branch_name.to_string()) {
    // Cycle detected — treat as no non-cleanable children to avoid infinite loop.
    return Ok(false);
  }

  let children = repo_state.get_dependency_children(branch_name);

  for child in children {
    if is_current_branch(repo, child)? {
      return Ok(true);
    }

    if has_unique_commits(repo, child, branch_name)? {
      return Ok(true);
    }

    if has_non_cleanable_children_inner(repo_state, repo, child, visited)? {
      return Ok(true);
    }
  }

  Ok(false)
}

/// Find a cleanable dependency chain starting from a branch.
///
/// # Returns
/// - `Some(vec)` — clean all branches in `vec` (the chain has ≥ 1 member and
///   is safe to delete as a unit).
/// - `None` — `start_branch` is an intermediate node with exactly one child
///   that has unique commits; skip it.
///
/// (Issue #12: clearer return-type semantics than returning an empty Vec)
fn find_cleanable_dependency_chain(
  repo_state: &RepoState,
  repo: &Git2Repository,
  start_branch: &str,
) -> Result<Option<Vec<String>>> {
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
    Ok(Some(chain))
  } else {
    // start_branch has exactly one child with unique commits — skip it.
    Ok(None)
  }
}

/// Find reparenting opportunities for aggressive cleanup.
///
/// Groups dependencies by their unique intermediate (parent) branch to avoid
/// producing conflicting operations when the same branch appears as a parent
/// in multiple dependency entries. (Issue #4)
///
/// # Returns
/// A list of `(child, old_parent, new_parent)` triples.
fn find_reparenting_opportunities(
  repo_state: &RepoState,
  repo: &Git2Repository,
) -> Result<Vec<(String, String, String)>> {
  // Collect unique intermediate branches (the `parent` side of dependencies).
  let unique_intermediates: HashSet<&str> = repo_state
    .dependencies
    .iter()
    .map(|d| d.parent.as_str())
    .collect();

  let mut reparenting_ops = Vec::new();

  for intermediate_branch in unique_intermediates {
    if is_current_branch(repo, intermediate_branch)? {
      continue;
    }

    let children = repo_state.get_dependency_children(intermediate_branch);

    // Only reparent when intermediate has exactly one child (unambiguous).
    if children.len() != 1 {
      continue;
    }

    let child_branch = children[0];
    // Issue #8: always use fallback parents here since this is aggressive mode.
    let grandparent = find_parent_branch(repo_state, repo, intermediate_branch, true)?;

    // Issue #5: collapse the two nested `if` into one (clippy::collapsible_if).
    if let Some(grandparent_name) = grandparent
      && !has_unique_commits(repo, intermediate_branch, &grandparent_name)?
    {
      reparenting_ops.push((
        child_branch.to_string(),
        intermediate_branch.to_string(),
        grandparent_name,
      ));
    }
  }

  Ok(reparenting_ops)
}

/// Find the parent branch for a given branch.
///
/// When `with_fallback` is `true` (used in `--aggressive` mode), also tries
/// common root branch names (`main`, `master`, `develop`) if the branch has no
/// explicitly tracked parent.  Without the flag only twig-tracked parents are
/// considered, preventing accidental deletion of unmanaged branches.
/// (Issue #8)
fn find_parent_branch(
  repo_state: &RepoState,
  repo: &Git2Repository,
  branch_name: &str,
  with_fallback: bool,
) -> Result<Option<String>> {
  let parents = repo_state.get_dependency_parents(branch_name);
  if let Some(parent) = parents.first() {
    return Ok(Some(parent.to_string()));
  }

  if with_fallback {
    // Fall back to common root branch names only in aggressive mode.
    let potential_parents = ["main", "master", "develop"];
    for parent in potential_parents {
      if repo.find_branch(parent, git2::BranchType::Local).is_ok() {
        return Ok(Some(parent.to_string()));
      }
    }
  }

  Ok(None)
}

/// Check if a branch has unique commits compared to its parent.
///
/// Returns `true` when `branch_name` contains at least one commit that is not
/// reachable from `parent_name`.
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

/// Delete a branch from the git repository.
fn delete_branch(repo: &Git2Repository, branch_name: &str) -> Result<()> {
  let mut branch = repo.find_branch(branch_name, git2::BranchType::Local)?;
  branch
    .delete()
    .with_context(|| format!("Failed to delete branch '{}'", branch_name))?;
  Ok(())
}

/// Clean up a branch from the twig configuration.
///
/// Removes all dependencies, root entries, *and* the branch metadata entry
/// (`repo_state.branches`) to keep the config consistent with `prune`.
/// (Issue #9)
fn cleanup_branch_from_config(repo_state: &mut RepoState, branch_name: &str) {
  let removed_dependencies = repo_state.remove_all_dependencies_for_branch(branch_name);
  let removed_from_roots = repo_state.remove_root(branch_name);
  // Issue #9: also remove branch metadata to stay consistent with `prune`.
  repo_state.branches.remove(branch_name);

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
  use twig_test_utils::{GitRepoTestGuard, create_branch, create_commit};

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

  /// Create a minimal initial commit so that branches can be created.
  fn make_initial_commit(guard: &GitRepoTestGuard) {
    create_commit(&guard.repo, "README.md", "init", "Initial commit")
      .expect("initial commit");
  }

  // -----------------------------------------------------------------------
  // Original edge-case tests (kept and updated for new return types)
  // -----------------------------------------------------------------------

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
    // A nonexistent branch with no children should return itself as a leaf.
    let chain = result.expect("chain").expect("should return Some");
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
    let chain = result.expect("chain").expect("should return Some");

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

  // -----------------------------------------------------------------------
  // Integration tests (Issue #10)
  // -----------------------------------------------------------------------

  #[test]
  fn test_has_unique_commits_same_tip() {
    let guard = GitRepoTestGuard::new();
    make_initial_commit(&guard);

    // Create feature off main — no extra commits, so tips are the same.
    create_branch(&guard.repo, "feature", None).expect("create branch");

    let result = has_unique_commits(&guard.repo, "feature", "main");
    assert!(result.is_ok());
    assert!(!result.expect("result"), "same tip → no unique commits");
  }

  #[test]
  fn test_has_unique_commits_with_extra_commit() {
    let guard = GitRepoTestGuard::new();
    make_initial_commit(&guard);
    create_branch(&guard.repo, "feature", None).expect("create branch");

    // Check out feature and add a commit.
    guard
      .repo
      .set_head("refs/heads/feature")
      .expect("set head to feature");
    create_commit(&guard.repo, "feat.txt", "new", "feat commit").expect("feat commit");

    let result = has_unique_commits(&guard.repo, "feature", "main");
    assert!(result.is_ok());
    assert!(result.expect("result"), "extra commit → unique commits");
  }

  #[test]
  fn test_is_current_branch_detached_head() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo = git2::Repository::init(temp_dir.path()).expect("init repo");

    // Freshly-initialised repo has an unborn HEAD — should return false, not Err.
    let result = is_current_branch(&repo, "main");
    assert!(result.is_ok(), "detached/unborn HEAD should not error");
    assert!(!result.expect("result"));
  }

  #[test]
  fn test_cleanup_branch_from_config_removes_metadata() {
    let mut state = RepoState::default();
    state
      .add_dependency("child".to_string(), "parent".to_string())
      .expect("add dep");
    // Simulate a branch metadata entry.
    state.branches.insert(
      "child".to_string(),
      twig_core::state::BranchMetadata {
        branch: "child".to_string(),
        jira_issue: None,
        github_pr: None,
        created_at: String::new(),
      },
    );

    cleanup_branch_from_config(&mut state, "child");

    assert!(
      !state.branches.contains_key("child"),
      "branch metadata must be removed (Issue #9)"
    );
    let children = state.get_dependency_children("parent");
    assert!(children.is_empty(), "dependency must be removed");
  }

  #[test]
  fn test_find_parent_branch_no_fallback_without_aggressive() {
    let guard = GitRepoTestGuard::new();
    make_initial_commit(&guard);

    // "orphan" has no twig parent.  Without the aggressive flag, the fallback
    // to main/master/develop must be disabled.
    create_branch(&guard.repo, "orphan", None).expect("create orphan");

    let state = RepoState::default();
    let result = find_parent_branch(&state, &guard.repo, "orphan", false);
    assert!(result.is_ok());
    assert!(
      result.expect("result").is_none(),
      "without aggressive, untracked branch should have no parent"
    );
  }

  #[test]
  fn test_find_parent_branch_fallback_with_aggressive() {
    let guard = GitRepoTestGuard::new();
    make_initial_commit(&guard);
    create_branch(&guard.repo, "orphan", None).expect("create orphan");

    let state = RepoState::default();
    let result = find_parent_branch(&state, &guard.repo, "orphan", true);
    assert!(result.is_ok());
    // main branch exists → the fallback should find it.
    assert_eq!(
      result.expect("result").as_deref(),
      Some("main"),
      "aggressive mode should fall back to main"
    );
  }

  #[test]
  fn test_has_non_cleanable_children_cycle_guard() {
    // Build a state with a dependency cycle (simulate corrupt state by
    // pushing directly to the vec).
    let mut state = RepoState::default();
    state
      .add_dependency("b".to_string(), "a".to_string())
      .expect("add dep a->b");
    state.dependencies.push(twig_core::state::BranchDependency {
      id: uuid::Uuid::new_v4(),
      child: "a".to_string(),
      parent: "b".to_string(),
      created_at: chrono::Utc::now(),
    });

    let temp_dir = TempDir::new().expect("temp dir");
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).expect("init bare");

    // The visited guard terminates the recursion.  The function may return
    // Ok or Err (branches "a"/"b" do not exist in the bare repo), but it
    // must NOT panic or stack-overflow.
    let _ = has_non_cleanable_children(&state, &mock_repo, "a");
    // If we reach this line the cycle guard worked correctly.
  }

  #[test]
  fn test_find_reparenting_no_conflicting_ops_for_shared_parent() {
    // An intermediate branch with two children must NOT be reparented
    // (ambiguous — Issue #4).
    let mut state = RepoState::default();
    state
      .add_dependency("child-a".to_string(), "intermediate".to_string())
      .expect("dep");
    state
      .add_dependency("child-b".to_string(), "intermediate".to_string())
      .expect("dep");
    state
      .add_dependency("intermediate".to_string(), "main".to_string())
      .expect("dep");

    let guard = GitRepoTestGuard::new();
    make_initial_commit(&guard);
    for name in ["intermediate", "child-a", "child-b"] {
      create_branch(&guard.repo, name, None).expect("create");
    }
    guard
      .repo
      .set_head("refs/heads/main")
      .expect("set head to main");

    let ops = find_reparenting_opportunities(&state, &guard.repo).expect("ops");
    assert!(
      ops.is_empty(),
      "ambiguous intermediate (2 children) must not produce a reparenting op"
    );
  }
}
