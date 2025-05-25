use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::Repository as Git2Repository;
use serde::{Deserialize, Serialize};

/// Represents a worktree in a repository
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Worktree {
  pub name: String,
  pub path: String,
  pub branch: String,
  pub created_at: String,
}

/// Represents a branch-issue association
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchIssue {
  pub branch: String,
  pub jira_issue: String,
  pub github_pr: Option<u32>,
  pub created_at: String,
}

/// Represents the repository-local state
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RepoState {
  pub version: u32,
  pub worktrees: Vec<Worktree>,
  pub branch_issues: Vec<BranchIssue>,
  pub config_overrides: serde_json::Value,
}

impl RepoState {
  /// Load the repository state from disk
  pub fn load<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
    // Use the ConfigDirs to get the state path
    let config_dirs = crate::config::ConfigDirs::new()?;
    let state_path = config_dirs.repo_state_path(&repo_path);

    if !state_path.exists() {
      return Ok(Self {
        version: 1,
        worktrees: Vec::new(),
        branch_issues: Vec::new(),
        config_overrides: serde_json::Value::Object(serde_json::Map::new()),
      });
    }

    let content = fs::read_to_string(&state_path).context("Failed to read state file")?;
    let state = serde_json::from_str(&content).context("Failed to parse state file")?;

    Ok(state)
  }

  /// Save the repository state to disk
  pub fn save<P: AsRef<Path>>(&self, repo_path: P) -> Result<()> {
    // Use the ConfigDirs to get the state directory and path
    let config_dirs = crate::config::ConfigDirs::new()?;
    let twig_dir = config_dirs.repo_state_dir(&repo_path);

    if !twig_dir.exists() {
      fs::create_dir_all(&twig_dir).context("Failed to create .twig directory")?;

      // Add .twig to .gitignore if it doesn't already contain it
      let gitignore_path = repo_path.as_ref().join(".gitignore");
      let mut gitignore_content = String::new();
      let mut needs_twig_entry = true;

      if gitignore_path.exists() {
        gitignore_content = fs::read_to_string(&gitignore_path).context("Failed to read .gitignore file")?;

        // Check if .twig is already in .gitignore
        if gitignore_content.lines().any(|line| line.trim() == ".twig/") {
          needs_twig_entry = false;
        }
      }

      if needs_twig_entry {
        // Ensure there's a newline before adding .twig/
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
          gitignore_content.push('\n');
        }

        gitignore_content.push_str(".twig/\n");
        fs::write(&gitignore_path, gitignore_content).context("Failed to update .gitignore file")?;
      }
    }

    let state_path = config_dirs.repo_state_path(&repo_path);
    let content = serde_json::to_string_pretty(self).context("Failed to serialize state")?;

    fs::write(&state_path, content).context("Failed to write state file")?;

    Ok(())
  }

  /// Add a worktree to the state
  pub fn add_worktree(&mut self, worktree: Worktree) {
    // Remove any existing worktree with the same name
    self.worktrees.retain(|w| w.name != worktree.name);
    self.worktrees.push(worktree);
  }

  /// Remove a worktree from the state
  pub fn remove_worktree(&mut self, name: &str) -> bool {
    let initial_len = self.worktrees.len();
    self.worktrees.retain(|w| w.name != name);
    self.worktrees.len() < initial_len
  }

  /// Get a worktree by name
  pub fn get_worktree(&self, name: &str) -> Option<&Worktree> {
    self.worktrees.iter().find(|w| w.name == name)
  }

  /// List all worktrees
  pub fn list_worktrees(&self) -> &[Worktree] {
    &self.worktrees
  }

  /// Add a branch-issue association
  pub fn add_branch_issue(&mut self, branch_issue: BranchIssue) {
    // Remove any existing association for the same branch
    self.branch_issues.retain(|bi| bi.branch != branch_issue.branch);
    self.branch_issues.push(branch_issue);
  }

  /// Get a branch-issue association by branch name
  pub fn get_branch_issue_by_branch(&self, branch: &str) -> Option<&BranchIssue> {
    self.branch_issues.iter().find(|bi| bi.branch == branch)
  }

  /// Get a branch-issue association by Jira issue key
  #[allow(dead_code)]
  pub fn get_branch_issue_by_jira(&self, jira_issue: &str) -> Option<&BranchIssue> {
    self.branch_issues.iter().find(|bi| bi.jira_issue == jira_issue)
  }

  /// List all branch-issue associations
  #[allow(dead_code)]
  pub fn list_branch_issues(&self) -> &[BranchIssue] {
    &self.branch_issues
  }
}

/// Create a new worktree
pub fn create_worktree<P: AsRef<Path>>(repo_path: P, branch_name: &str) -> Result<PathBuf> {
  use crate::utils::output::{format_repo_path, print_info, print_success, print_warning};

  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Determine the worktree path
  // By default, create worktrees in a directory named after the repo with
  // "-worktrees" suffix
  let repo_name = repo_path.file_name().and_then(|n| n.to_str()).unwrap_or("repo");

  let parent_dir = repo_path.parent().unwrap_or(Path::new("."));
  let worktrees_dir = parent_dir.join(format!("{repo_name}-worktrees"));

  // Create the worktrees directory if it doesn't exist
  if !worktrees_dir.exists() {
    fs::create_dir_all(&worktrees_dir).context(format!(
      "Failed to create worktrees directory at {}",
      worktrees_dir.display()
    ))?;
  }

  // Sanitize branch name for use as directory name
  let safe_branch_name = branch_name.replace('/', "-");
  let worktree_path = worktrees_dir.join(&safe_branch_name);

  print_info(&format!(
    "Creating worktree at {}",
    format_repo_path(&worktree_path.display().to_string())
  ));

  // Check if branch exists
  let branch_exists = repo.find_branch(branch_name, git2::BranchType::Local).is_ok();

  // Also check if a branch with the sanitized name exists (could happen with
  // previous attempts)
  let sanitized_branch_exists = repo.find_branch(&safe_branch_name, git2::BranchType::Local).is_ok();

  if branch_exists {
    // Use existing branch
    print_info(&format!("Using existing branch: {branch_name}"));
    // Check if the worktree directory already exists
    if worktree_path.exists() {
      print_warning(&format!(
        "Worktree directory already exists at {}",
        format_repo_path(&worktree_path.display().to_string())
      ));
      return Err(anyhow::anyhow!(
        "Worktree directory already exists at {}. Please remove it or use a different branch name.",
        worktree_path.display()
      ));
    }

    // Check if a worktree with this name already exists
    if repo.find_worktree(&safe_branch_name).is_ok() {
      print_warning(&format!("A worktree named '{safe_branch_name}' already exists",));
      return Err(anyhow::anyhow!(
        "A worktree named '{}' already exists. This could be due to a previous attempt to create this worktree.",
        safe_branch_name
      ));
    }

    // Check if a branch with the sanitized name exists (would conflict with
    // worktree creation)
    if sanitized_branch_exists {
      print_warning(&format!(
        "A branch named '{safe_branch_name}' already exists, which conflicts with the worktree name",
      ));
      return Err(anyhow::anyhow!(
        "A branch named '{}' already exists, which conflicts with the worktree name. Please delete this branch first or use a different branch name.",
        safe_branch_name
      ));
    }

    // Try to create the worktree
    match repo.worktree(safe_branch_name.as_str(), worktree_path.as_path(), None) {
      Ok(worktree) => worktree,
      Err(err) => {
        // Get the raw error message from git2
        let git_error = err.message();

        return Err(anyhow::anyhow!(
          "Failed to create worktree for branch '{}': {}. This could be due to:
  - The worktree directory already exists but is not registered with Git
  - The branch is already checked out in another worktree
  - There are uncommitted changes that conflict with the branch
  - You don't have permission to create directories at {}",
          branch_name,
          git_error,
          worktree_path.parent().unwrap_or(Path::new(".")).display()
        ));
      }
    };
  } else {
    // Create a new branch
    print_info(&format!("Creating new branch: {branch_name}"));

    // Get the HEAD commit to branch from
    let head = repo.head()?;
    let target = head
      .target()
      .ok_or_else(|| anyhow::anyhow!("HEAD is not a direct reference"))?;
    let commit = repo.find_commit(target)?;

    // Create the branch
    repo
      .branch(branch_name, &commit, false)
      .context(format!("Failed to create branch '{branch_name}'"))?;

    // Create the worktree
    // Check if the worktree directory already exists
    if worktree_path.exists() {
      print_warning(&format!(
        "Worktree directory already exists at {}",
        format_repo_path(&worktree_path.display().to_string())
      ));
      return Err(anyhow::anyhow!(
        "Worktree directory already exists at {}. Please remove it or use a different branch name.",
        worktree_path.display()
      ));
    }

    // Check if a worktree with this name already exists
    if repo.find_worktree(&safe_branch_name).is_ok() {
      print_warning(&format!("A worktree named '{safe_branch_name}' already exists",));
      return Err(anyhow::anyhow!(
        "A worktree named '{safe_branch_name}' already exists. This could be due to a previous attempt to create this worktree.",
      ));
    }

    // Check if a branch with the sanitized name exists (would conflict with
    // worktree creation)
    if repo.find_branch(&safe_branch_name, git2::BranchType::Local).is_ok() {
      print_warning(&format!(
        "A branch named '{safe_branch_name}' already exists, which conflicts with the worktree name",
      ));
      return Err(anyhow::anyhow!(
        "A branch named '{safe_branch_name}' already exists, which conflicts with the worktree name. Please delete this branch first or use a different branch name.",
      ));
    }

    // Try to create the worktree
    match repo.worktree(safe_branch_name.as_str(), worktree_path.as_path(), None) {
      Ok(worktree) => worktree,
      Err(err) => {
        // Get the raw error message from git2
        let git_error = err.message();

        return Err(anyhow::anyhow!(
          "Failed to create worktree for branch '{}': {}. This could be due to:
  - The worktree directory already exists but is not registered with Git
  - The branch is already checked out in another worktree
  - There are uncommitted changes that conflict with the branch
  - You don't have permission to create directories at {}",
          branch_name,
          git_error,
          worktree_path.parent().unwrap_or(Path::new(".")).display()
        ));
      }
    };
  }

  // Update the repository state
  let mut state = RepoState::load(repo_path)?;

  // Get current timestamp
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
    .unwrap()
    .to_rfc3339();

  // Add the worktree to the state
  state.add_worktree(Worktree {
    name: safe_branch_name,
    path: worktree_path.to_string_lossy().to_string(),
    branch: branch_name.to_string(),
    created_at: time_str,
  });

  state.save(repo_path)?;

  print_success(&format!(
    "Successfully created worktree for branch '{}' at {}",
    branch_name,
    format_repo_path(&worktree_path.display().to_string())
  ));

  Ok(worktree_path)
}

/// List all worktrees for a repository
pub fn list_worktrees<P: AsRef<Path>>(repo_path: P) -> Result<()> {
  use crate::utils::output::{
    format_command, format_repo_path, format_timestamp, print_header, print_info, print_warning,
  };

  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the list of worktrees from git
  let worktree_names = repo.worktrees()?;

  if worktree_names.is_empty() {
    print_warning("No worktrees found for this repository.");
    print_info(&format!(
      "Create one with {}",
      format_command("twig worktree create <branch-name>")
    ));
    return Ok(());
  }

  // Load the repository state to get additional metadata
  let state = RepoState::load(repo_path)?;

  print_header("Worktrees");

  // Get all worktrees from the state
  let state_worktrees = state.list_worktrees();

  // Iterate through the worktree names
  for i in 0..worktree_names.len() {
    if let Some(name) = worktree_names.get(i) {
      let worktree = repo.find_worktree(name)?;
      let path = worktree.path().to_string_lossy().to_string();

      // Try to get additional metadata from the state
      let state_worktree = state.get_worktree(name);

      println!("  Branch: {name}",);
      println!("  Path: {}", format_repo_path(&path));

      if let Some(wt) = state_worktree {
        println!("  Created: {}", format_timestamp(&wt.created_at));
      } else {
        // If we don't have metadata in the state, check if we have any worktrees in the
        // state
        if !state_worktrees.is_empty() {
          println!("  Created: Unknown (no metadata available)");
        }
      }

      println!();
    }
  }

  Ok(())
}

/// Clean up stale worktrees
pub fn clean_worktrees<P: AsRef<Path>>(repo_path: P) -> Result<()> {
  use crate::utils::output::{print_info, print_success, print_warning};

  let repo_path = repo_path.as_ref();
  let repo =
    Git2Repository::open(repo_path).context(format!("Failed to open git repository at {}", repo_path.display()))?;

  // Get the list of worktrees from git
  let worktree_names = repo.worktrees()?;

  if worktree_names.is_empty() {
    print_warning("No worktrees found for this repository.");
    return Ok(());
  }

  // Load the repository state
  let mut state = RepoState::load(repo_path)?;
  let mut cleaned_count = 0;

  // Iterate through the worktree names
  for i in 0..worktree_names.len() {
    if let Some(name) = worktree_names.get(i) {
      let worktree = repo.find_worktree(name)?;
      let path = worktree.path();

      // Check if the worktree directory still exists
      if !path.exists() {
        print_info(&format!(
          "Cleaning up stale worktree reference: {name} (path no longer exists)",
        ));

        // Prune the worktree reference
        worktree.prune(None)?;

        // Remove from state
        state.remove_worktree(name);

        cleaned_count += 1;
      }
    }
  }

  // Save the updated state
  state.save(repo_path)?;

  if cleaned_count > 0 {
    print_success(&format!("Cleaned up {cleaned_count} stale worktree references"));
  } else {
    print_info("No stale worktrees found to clean up");
  }

  Ok(())
}
