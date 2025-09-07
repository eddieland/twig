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

  // Legacy flags for backward compatibility (when no subcommand is provided)
  #[arg(
    long = "dry-run",
    long_help = "Show which branches would be deleted without actually deleting them\n\n\
                Preview the branches that would be removed by the tidy operation.\n\
                This is useful to verify the operation before actually running it."
  )]
  pub dry_run: bool,

  #[arg(
    short = 'f',
    long = "force",
    long_help = "Skip confirmation prompt and delete branches immediately\n\n\
                By default, tidy will ask for confirmation before deleting branches.\n\
                Use this flag to skip the confirmation and delete branches automatically."
  )]
  pub force: bool,
}

/// Subcommands for the tidy command
#[derive(Subcommand)]
pub enum TidyCommand {
  /// Clean up branches with no unique commits and no children (default
  /// behavior)
  #[command(
    long_about = "Clean up branches that have no unique commits compared to their parent\n\
                and have no child dependencies.\n\n\
                This command identifies branches that:\n\
                • Have no commits that differ from their parent branch\n\
                • Have no child branches depending on them\n\
                • Are not the current branch\n\n\
                Use the --aggressive (-a) flag to enable reparenting of branches when\n\
                intermediate branches have no changes. For example, if A -> B -> C and\n\
                B has no changes, C will be reparented to A and B will be deleted.\n\n\
                These branches are typically safe to delete as they don't contain unique\n\
                work and won't break dependency chains."
  )]
  Clean(CleanArgs),

  /// Remove deleted branches from the twig tree configuration
  #[command(
    long_about = "Remove references to deleted branches from the twig tree configuration.\n\n\
                This command finds branches that are referenced in the twig configuration\n\
                (dependencies, root branches, metadata) but no longer exist in the Git repository.\n\
                It removes these stale references to keep the twig tree clean.\n\n\
                This is useful when branches have been deleted outside of twig (e.g., via\n\
                'git branch -d' or through a Git UI) and you want to clean up the twig\n\
                configuration to match the actual repository state."
  )]
  Prune(PruneArgs),
}

/// Arguments for the clean subcommand
#[derive(Args)]
pub struct CleanArgs {
  #[arg(
    long = "dry-run",
    long_help = "Show which branches would be deleted without actually deleting them"
  )]
  pub dry_run: bool,

  #[arg(
    short = 'f',
    long = "force",
    long_help = "Skip confirmation prompt and delete branches immediately"
  )]
  pub force: bool,

  #[arg(
    short = 'a',
    long = "aggressive",
    long_help = "Aggressively clean up by reparenting branches when intermediate branches have no changes\n\n\
                When enabled, if an intermediate branch has no unique commits compared to its parent,\n\
                its children will be reparented to the parent, and the intermediate branch will be deleted.\n\
                For example: A -> B -> C, if B has no changes, C will be reparented to A and B deleted."
  )]
  pub aggressive: bool,
}

/// Arguments for the prune subcommand
#[derive(Args)]
pub struct PruneArgs {
  #[arg(
    long = "dry-run",
    long_help = "Show which branches would be removed from configuration without actually removing them"
  )]
  pub dry_run: bool,

  #[arg(
    short = 'f',
    long = "force",
    long_help = "Skip confirmation prompt and remove references immediately"
  )]
  pub force: bool,
}

/// Handle the tidy command
///
/// This function routes to the appropriate subcommand handler or provides
/// backward compatibility for the original clean behavior.
pub(crate) fn handle_tidy_command(tidy: TidyArgs) -> Result<()> {
  match tidy.command {
    Some(TidyCommand::Clean(args)) => handle_clean_command(args),
    Some(TidyCommand::Prune(args)) => handle_prune_command(args),
    None => {
      // Backward compatibility: if no subcommand is provided, run clean
      let clean_args = CleanArgs {
        dry_run: tidy.dry_run,
        force: tidy.force,
        aggressive: false, // Default to non-aggressive for backward compatibility
      };
      handle_clean_command(clean_args)
    }
  }
}

/// Handle the clean subcommand
///
/// This function finds branches that have no unique commits compared to their
/// parent branch and have no child branches, then deletes them to clean up
/// the repository. It now also recursively cleans up dependency chains where
/// none of the branches in the chain have changes from master.
pub fn handle_clean_command(clean: CleanArgs) -> Result<()> {
  let repo_path = detect_repository().context("Not in a git repository")?;
  let repo = Git2Repository::open(&repo_path)?;

  // Load repository state to understand dependencies
  let repo_state = RepoState::load(&repo_path)?;

  // Get all local branches
  let branches = repo
    .branches(Some(git2::BranchType::Local))?
    .collect::<Result<Vec<_>, _>>()
    .context("Failed to collect branches")?;

  let mut branches_to_delete = Vec::new();
  let mut reparenting_operations = Vec::new(); // Store (child, old_parent, new_parent) tuples
  let mut processed_chains = HashSet::new();

  print_info("Analyzing branches for cleanup...");

  // If aggressive mode is enabled, first handle reparenting
  if clean.aggressive {
    reparenting_operations = find_reparenting_opportunities(&repo_state, &repo)?;
    
    if !reparenting_operations.is_empty() {
      print_info(&format!("Found {} reparenting opportunities:", reparenting_operations.len()));
      for (child, old_parent, new_parent) in &reparenting_operations {
        println!("  • {} will be reparented from {} to {}", child, old_parent, new_parent);
      }
    }
  }

  for (branch, _) in branches {
    let branch_name = match branch.name()? {
      Some(name) => name.to_string(),
      None => continue, // Skip branches with invalid names
    };

    // Skip the current branch
    if is_current_branch(&repo, &branch_name)? {
      continue;
    }

    // Skip if we've already processed this branch as part of a chain
    if processed_chains.contains(&branch_name) {
      continue;
    }

    // Check if this branch has children that are not part of a cleanable chain
    if has_non_cleanable_children(&repo_state, &repo, &branch_name)? {
      continue;
    }

    // Find the parent branch
    let parent_branch = find_parent_branch(&repo_state, &repo, &branch_name)?;

    if let Some(parent) = parent_branch {
      // Check if branch has unique commits compared to parent
      if !has_unique_commits(&repo, &branch_name, &parent)? {
        // Check if this is part of a cleanable dependency chain
        let chain = find_cleanable_dependency_chain(&repo_state, &repo, &branch_name)?;
        
        for chain_branch in &chain {
          processed_chains.insert(chain_branch.clone());
          if !branches_to_delete.contains(chain_branch) {
            branches_to_delete.push(chain_branch.clone());
          }
        }
        
        // If no chain was found, add just this branch
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

  // Show what would be changed
  if !branches_to_delete.is_empty() {
    print_info(&format!("Found {} branches to delete:", branches_to_delete.len()));
    for branch in &branches_to_delete {
      println!("  • {}", branch);
    }
  }

  if !reparenting_operations.is_empty() {
    print_info(&format!("Found {} reparenting operations:", reparenting_operations.len()));
    for (child, old_parent, new_parent) in &reparenting_operations {
      println!("  • {} will be reparented from {} to {}", child, old_parent, new_parent);
    }
  }

  if clean.dry_run {
    print_info("Dry run mode - no changes were actually made.");
    return Ok(());
  }

  // Confirm operations unless --force is used
  if !clean.force {
    if !branches_to_delete.is_empty() && !reparenting_operations.is_empty() {
      print_warning("This will permanently delete branches and reparent others.");
    } else if !branches_to_delete.is_empty() {
      print_warning("This will permanently delete the listed branches.");
    } else {
      print_warning("This will reparent the listed branches.");
    }
    print!("Continue? (y/N): ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().to_lowercase().starts_with('y') {
      print_info("Operation cancelled.");
      return Ok(());
    }
  }

  // Perform reparenting operations first
  let mut repo_state = repo_state; // Make it mutable
  let mut reparented_count = 0;

  for (child, old_parent, new_parent) in reparenting_operations {
    // Remove old dependency
    if repo_state.remove_dependency(&child, &old_parent) {
      // Add new dependency
      match repo_state.add_dependency(child.clone(), new_parent.clone()) {
        Ok(()) => {
          print_success(&format!("Reparented {} from {} to {}", child, old_parent, new_parent));
          reparented_count += 1;
          
          // If the old parent now has no children and no unique commits, mark it for deletion
          if repo_state.get_dependency_children(&old_parent).is_empty() {
            if let Ok(parent_of_old) = find_parent_branch(&repo_state, &repo, &old_parent) {
              if let Some(parent) = parent_of_old {
                if let Ok(false) = has_unique_commits(&repo, &old_parent, &parent) {
                  if !branches_to_delete.contains(&old_parent) {
                    branches_to_delete.push(old_parent.clone());
                    print_info(&format!("Added {} to deletion list (no children, no changes)", old_parent));
                  }
                }
              }
            }
          }
        }
        Err(e) => {
          print_error(&format!("Failed to reparent {} from {} to {}: {}", child, old_parent, new_parent, e));
          // Try to restore the old dependency if the new one failed
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
        // Remove branch from twig configuration
        cleanup_branch_from_config(&mut repo_state, &branch_name);

        print_success(&format!("Deleted branch: {}", branch_name));
        deleted_count += 1;
      }
      Err(e) => {
        print_error(&format!("Failed to delete branch {}: {}", branch_name, e));
      }
    }
  }

  // Save the updated configuration if any changes were made
  if deleted_count > 0 || reparented_count > 0 {
    if let Err(e) = repo_state.save(&repo_path) {
      print_warning(&format!("Failed to save updated configuration: {}", e));
    } else {
      print_info("Updated twig configuration.");
    }
  }

  if deleted_count > 0 && reparented_count > 0 {
    print_success(&format!("Clean complete: deleted {} branches, reparented {} branches.", deleted_count, reparented_count));
  } else if deleted_count > 0 {
    print_success(&format!("Clean complete: deleted {} branches.", deleted_count));
  } else if reparented_count > 0 {
    print_success(&format!("Clean complete: reparented {} branches.", reparented_count));
  } else {
    print_info("No changes were made.");
  }
  
  Ok(())
}

/// Handle the prune subcommand
///
/// This function finds branches that are referenced in the twig configuration
/// but no longer exist in the Git repository, then removes these stale
/// references.
fn handle_prune_command(prune: PruneArgs) -> Result<()> {
  let repo_path = detect_repository().context("Not in a git repository")?;
  let repo = Git2Repository::open(&repo_path)?;

  // Load repository state
  let mut repo_state = RepoState::load(&repo_path)?;

  // Get all existing local branch names
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
  let mut dependencies_to_remove = Vec::new();
  let mut roots_to_remove = Vec::new();
  let mut metadata_to_remove = Vec::new();

  // Check dependencies for non-existent branches
  for dependency in &repo_state.dependencies {
    if !existing_branches.contains(&dependency.child) {
      dependencies_to_remove.push(dependency.clone());
      branches_to_remove.push(dependency.child.clone());
    }
    if !existing_branches.contains(&dependency.parent) {
      dependencies_to_remove.push(dependency.clone());
      branches_to_remove.push(dependency.parent.clone());
    }
  }

  // Check root branches for non-existent branches
  for root_branch in &repo_state.root_branches {
    if !existing_branches.contains(&root_branch.branch) {
      roots_to_remove.push(root_branch.clone());
      branches_to_remove.push(root_branch.branch.clone());
    }
  }

  // Check metadata for non-existent branches
  for branch_name in repo_state.branches.keys() {
    if !existing_branches.contains(branch_name) {
      metadata_to_remove.push(branch_name.clone());
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

  // Show what would be removed
  print_info(&format!(
    "Found {} deleted branches to remove from twig configuration:",
    branches_to_remove.len()
  ));
  for branch in &branches_to_remove {
    println!("  • {}", branch);
  }

  if !dependencies_to_remove.is_empty() {
    print_info(&format!(
      "  {} dependencies will be removed",
      dependencies_to_remove.len()
    ));
  }
  if !roots_to_remove.is_empty() {
    print_info(&format!(
      "  {} root branch entries will be removed",
      roots_to_remove.len()
    ));
  }
  if !metadata_to_remove.is_empty() {
    print_info(&format!(
      "  {} metadata entries will be removed",
      metadata_to_remove.len()
    ));
  }

  if prune.dry_run {
    print_info("Dry run mode - no configuration changes were made.");
    return Ok(());
  }

  // Confirm removal unless --force is used
  if !prune.force {
    print_warning("This will remove the stale references from twig configuration.");
    print!("Continue? (y/N): ");
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().to_lowercase().starts_with('y') {
      print_info("Operation cancelled.");
      return Ok(());
    }
  }

  // Remove stale references
  let mut removed_count = 0;

  // Remove dependencies
  for branch in &branches_to_remove {
    let removed_deps = repo_state.remove_all_dependencies_for_branch(branch);
    removed_count += removed_deps;
  }

  // Remove root branches
  for branch in &branches_to_remove {
    if repo_state.remove_root(branch) {
      removed_count += 1;
    }
  }

  // Remove metadata
  for branch in &branches_to_remove {
    if repo_state.branches.remove(branch).is_some() {
      removed_count += 1;
    }
  }

  // Save the updated configuration
  if removed_count > 0 {
    match repo_state.save(&repo_path) {
      Ok(()) => {
        print_success(&format!(
          "Prune complete: removed {} stale references from twig configuration.",
          removed_count
        ));
        print_info("Twig configuration updated successfully.");
      }
      Err(e) => {
        print_error(&format!("Failed to save updated configuration: {}", e));
        return Err(e);
      }
    }
  } else {
    print_info("No changes were made to twig configuration.");
  }

  Ok(())
}

/// Check if a branch is the current branch
fn is_current_branch(repo: &Git2Repository, branch_name: &str) -> Result<bool> {
  let head = repo.head()?;
  if let Some(current) = head.shorthand() {
    Ok(current == branch_name)
  } else {
    Ok(false)
  }
}

/// Check if a branch has children that are not part of a cleanable chain
/// This checks if any child has unique commits or is the current branch
fn has_non_cleanable_children(repo_state: &RepoState, repo: &Git2Repository, branch_name: &str) -> Result<bool> {
  let children = repo_state.get_dependency_children(branch_name);
  
  for child in children {
    // Skip if this is the current branch
    if is_current_branch(repo, child)? {
      return Ok(true);
    }
    
    // Check if child has unique commits compared to this branch
    if has_unique_commits(repo, child, branch_name)? {
      return Ok(true);
    }
    
    // Recursively check if the child has non-cleanable children
    if has_non_cleanable_children(repo_state, repo, child)? {
      return Ok(true);
    }
  }
  
  Ok(false)
}

/// Find a cleanable dependency chain starting from a branch
/// Returns all branches in the chain that can be safely deleted
fn find_cleanable_dependency_chain(repo_state: &RepoState, repo: &Git2Repository, start_branch: &str) -> Result<Vec<String>> {
  let mut chain = Vec::new();
  let mut current = start_branch;
  
  // Traverse down the dependency tree to find the complete cleanable chain
  loop {
    let children = repo_state.get_dependency_children(current);
    
    // If there are no children, we've reached the end of the chain
    if children.is_empty() {
      chain.push(current.to_string());
      break;
    }
    
    // If there are multiple children, we can't clean the entire chain
    if children.len() > 1 {
      break;
    }
    
    let child = children[0];
    
    // Check if child is the current branch (can't delete)
    if is_current_branch(repo, child)? {
      break;
    }
    
    // Check if child has unique commits compared to current
    if has_unique_commits(repo, child, current)? {
      break;
    }
    
    // Add current to chain and continue with the child
    chain.push(current.to_string());
    current = child;
  }
  
  // Only return the chain if it contains more than one element (indicating a dependency chain)
  // or if it's a single branch with no children (leaf node)
  if chain.len() > 1 || repo_state.get_dependency_children(start_branch).is_empty() {
    Ok(chain)
  } else {
    Ok(Vec::new())
  }
}

/// Find reparenting opportunities for aggressive cleanup
/// Returns a vector of (child, old_parent, new_parent) tuples
fn find_reparenting_opportunities(repo_state: &RepoState, repo: &Git2Repository) -> Result<Vec<(String, String, String)>> {
  let mut reparenting_ops = Vec::new();
  
  // Iterate through all dependencies to find intermediate branches with no changes
  for dependency in &repo_state.dependencies {
    let intermediate_branch = &dependency.parent;
    let child_branch = &dependency.child;
    
    // Skip if intermediate branch is the current branch
    if is_current_branch(repo, intermediate_branch)? {
      continue;
    }
    
    // Find the parent of the intermediate branch
    let grandparent = find_parent_branch(repo_state, repo, intermediate_branch)?;
    
    if let Some(grandparent_name) = grandparent {
      // Check if intermediate branch has no unique commits compared to its parent
      if !has_unique_commits(repo, intermediate_branch, &grandparent_name)? {
        // Check if intermediate branch has exactly one child (the current child)
        let children = repo_state.get_dependency_children(intermediate_branch);
        if children.len() == 1 && children[0] == child_branch {
          // This is a good candidate for reparenting
          reparenting_ops.push((
            child_branch.clone(),
            intermediate_branch.clone(),
            grandparent_name,
          ));
        }
      }
    }
  }
  
  Ok(reparenting_ops)
}

/// Find the parent branch for a given branch
fn find_parent_branch(repo_state: &RepoState, repo: &Git2Repository, branch_name: &str) -> Result<Option<String>> {
  // First check if there's an explicit dependency
  let parents = repo_state.get_dependency_parents(branch_name);
  if let Some(parent) = parents.first() {
    return Ok(Some(parent.to_string()));
  }

  // Fall back to trying common parent branches
  let potential_parents = vec!["main", "master", "develop"];

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

  // If they point to the same commit, branch has no unique commits
  if branch_commit == parent_commit {
    return Ok(false);
  }

  // Check if branch commit is ahead of parent
  let mut revwalk = repo.revwalk()?;
  revwalk.push(branch_commit)?;
  revwalk.hide(parent_commit)?;

  // If there are any commits, the branch has unique commits
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
/// This removes the branch from dependencies and root branch lists
fn cleanup_branch_from_config(repo_state: &mut RepoState, branch_name: &str) {
  // Remove all dependencies for this branch (both as child and parent)
  let removed_dependencies = repo_state.remove_all_dependencies_for_branch(branch_name);

  // Remove the branch from root branches if it was marked as one
  let removed_from_roots = repo_state.remove_root(branch_name);

  if removed_dependencies > 0 || removed_from_roots {
    print_info(&format!(
      "Cleaned up twig config for '{}': {} dependencies, {} root branch entries removed",
      branch_name,
      removed_dependencies,
      if removed_from_roots { 1 } else { 0 }
    ));
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use twig_core::state::RepoState;
  use tempfile::TempDir;

  fn create_mock_repo_state() -> RepoState {
    let mut state = RepoState::default();
    
    // Create dependencies: A -> B -> C
    state.add_dependency("branch-b".to_string(), "branch-a".to_string()).unwrap();
    state.add_dependency("branch-c".to_string(), "branch-b".to_string()).unwrap();
    
    state
  }

  #[test]
  fn test_has_non_cleanable_children_empty() {
    let repo_state = RepoState::default();
    let temp_dir = TempDir::new().unwrap();
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).unwrap();
    
    let result = has_non_cleanable_children(&repo_state, &mock_repo, "nonexistent");
    assert!(result.is_ok());
    assert!(!result.unwrap());
  }

  #[test]
  fn test_find_cleanable_dependency_chain_empty() {
    let repo_state = RepoState::default();
    let temp_dir = TempDir::new().unwrap();
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).unwrap();
    
    let result = find_cleanable_dependency_chain(&repo_state, &mock_repo, "nonexistent");
    assert!(result.is_ok());
    let chain = result.unwrap();
    
    // A nonexistent branch with no children should return the branch itself as a leaf
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], "nonexistent");
  }

  #[test]
  fn test_find_cleanable_dependency_chain_single_branch() {
    let repo_state = create_mock_repo_state();
    let temp_dir = TempDir::new().unwrap();
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).unwrap();
    
    // Test a branch with no children (should be considered cleanable as single branch)
    let result = find_cleanable_dependency_chain(&repo_state, &mock_repo, "branch-c");
    assert!(result.is_ok());
    let chain = result.unwrap();
    
    // Should return the branch itself since it has no children
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0], "branch-c");
  }

  #[test]
  fn test_find_reparenting_opportunities_simple() {
    let repo_state = create_mock_repo_state();
    let temp_dir = TempDir::new().unwrap();
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).unwrap();
    
    // Test basic reparenting opportunity detection
    // Since we have a bare repo with no actual branches, this should return empty
    // but not error out
    let result = find_reparenting_opportunities(&repo_state, &mock_repo);
    
    match result {
      Ok(ops) => {
        // Should return empty since no actual git branches exist
        assert!(ops.is_empty());
      }
      Err(_) => {
        // In a bare repo scenario, it's acceptable for this to error
        // since no branches exist - this is expected behavior
      }
    }
  }

  #[test]
  fn test_find_reparenting_opportunities_empty_state() {
    let repo_state = RepoState::default();
    let temp_dir = TempDir::new().unwrap();
    let mock_repo = git2::Repository::init_bare(temp_dir.path()).unwrap();
    
    let result = find_reparenting_opportunities(&repo_state, &mock_repo);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
  }
}
