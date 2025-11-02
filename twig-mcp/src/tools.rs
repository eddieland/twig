//! Tool implementations for branch tree management

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use git2::Repository;
use serde_json::{Value, json};
use twig_core::config::ConfigDirs;
use twig_core::state::{Registry, RepoState};

use crate::protocol::{CallToolParams, CallToolResult, Tool, ToolContent};

/// Get all available tools
pub fn get_tools() -> Vec<Tool> {
  vec![
    Tool {
      name: "twig_list_branches".to_string(),
      description: "List all branches in the current repository with their dependencies and metadata".to_string(),
      input_schema: json!({
          "type": "object",
          "properties": {
              "repo_path": {
                  "type": "string",
                  "description": "Path to the git repository (optional, defaults to current directory)"
              }
          },
          "required": []
      }),
    },
    Tool {
      name: "twig_get_branch_tree".to_string(),
      description: "Get the branch dependency tree visualization for the repository".to_string(),
      input_schema: json!({
          "type": "object",
          "properties": {
              "repo_path": {
                  "type": "string",
                  "description": "Path to the git repository"
              },
              "branch": {
                  "type": "string",
                  "description": "Starting branch (optional, defaults to current branch)"
              }
          },
          "required": []
      }),
    },
    Tool {
      name: "twig_get_branch_info".to_string(),
      description: "Get detailed information about a specific branch including Jira issue and GitHub PR".to_string(),
      input_schema: json!({
          "type": "object",
          "properties": {
              "repo_path": {
                  "type": "string",
                  "description": "Path to the git repository"
              },
              "branch": {
                  "type": "string",
                  "description": "Branch name"
              }
          },
          "required": ["branch"]
      }),
    },
    Tool {
      name: "twig_get_worktrees".to_string(),
      description: "List all worktrees in the repository".to_string(),
      input_schema: json!({
          "type": "object",
          "properties": {
              "repo_path": {
                  "type": "string",
                  "description": "Path to the git repository"
              }
          },
          "required": []
      }),
    },
    Tool {
      name: "twig_get_registry".to_string(),
      description: "Get all repositories registered with twig".to_string(),
      input_schema: json!({
          "type": "object",
          "properties": {},
          "required": []
      }),
    },
  ]
}

/// Execute a tool call
pub async fn call_tool(params: CallToolParams) -> Result<CallToolResult> {
  match params.name.as_str() {
    "twig_list_branches" => list_branches(params.arguments).await,
    "twig_get_branch_tree" => get_branch_tree(params.arguments).await,
    "twig_get_branch_info" => get_branch_info(params.arguments).await,
    "twig_get_worktrees" => get_worktrees(params.arguments).await,
    "twig_get_registry" => get_registry(params.arguments).await,
    _ => Err(anyhow!("Unknown tool: {}", params.name)),
  }
}

async fn list_branches(args: Option<Value>) -> Result<CallToolResult> {
  let repo_path = get_repo_path(args)?;
  let repo =
    Repository::open(&repo_path).with_context(|| format!("Failed to open repository at {}", repo_path.display()))?;

  let state = RepoState::load(&repo_path)?;
  let mut branches = Vec::new();

  // Get all local branches
  let branch_refs = repo.branches(Some(git2::BranchType::Local))?;
  for branch in branch_refs {
    let (branch, _) = branch?;
    if let Some(name) = branch.name()? {
      let metadata = state.get_branch_metadata(name);
      let info = json!({
          "name": name,
          "jira_issue": metadata.and_then(|m| m.jira_issue.clone()),
          "github_pr": metadata.and_then(|m| m.github_pr),
          "created_at": metadata.map(|m| m.created_at.clone()),
      });
      branches.push(info);
    }
  }

  Ok(CallToolResult {
    content: vec![ToolContent::Text {
      text: serde_json::to_string_pretty(&json!({
          "branches": branches,
          "total": branches.len()
      }))?,
    }],
    is_error: None,
  })
}

async fn get_branch_tree(args: Option<Value>) -> Result<CallToolResult> {
  let repo_path = get_repo_path(args.clone())?;
  let repo = Repository::open(&repo_path)?;

  let branch_name = args
    .and_then(|v| v.get("branch").and_then(|b| b.as_str()).map(String::from))
    .or_else(|| repo.head().ok().and_then(|head| head.shorthand().map(String::from)));

  let state = RepoState::load(&repo_path)?;
  let mut tree_info = Vec::new();

  // Build a simple dependency tree
  for (branch, metadata) in &state.branches {
    tree_info.push(json!({
        "branch": branch,
        "jira_issue": metadata.jira_issue,
        "github_pr": metadata.github_pr,
    }));
  }

  Ok(CallToolResult {
    content: vec![ToolContent::Text {
      text: serde_json::to_string_pretty(&json!({
          "current_branch": branch_name,
          "tree": tree_info
      }))?,
    }],
    is_error: None,
  })
}

async fn get_branch_info(args: Option<Value>) -> Result<CallToolResult> {
  let args = args.ok_or_else(|| anyhow!("Missing arguments"))?;
  let branch = args
    .get("branch")
    .and_then(|b| b.as_str())
    .ok_or_else(|| anyhow!("Missing branch parameter"))?
    .to_string();

  let repo_path = get_repo_path(Some(args))?;
  let state = RepoState::load(&repo_path)?;

  let metadata = state
    .get_branch_metadata(&branch)
    .ok_or_else(|| anyhow!("Branch {} not found in twig state", branch))?;

  Ok(CallToolResult {
    content: vec![ToolContent::Text {
      text: serde_json::to_string_pretty(&json!({
          "branch": metadata.branch,
          "jira_issue": metadata.jira_issue,
          "github_pr": metadata.github_pr,
          "created_at": metadata.created_at,
      }))?,
    }],
    is_error: None,
  })
}

async fn get_worktrees(args: Option<Value>) -> Result<CallToolResult> {
  let repo_path = get_repo_path(args)?;
  let repo = Repository::open(&repo_path)?;

  let mut worktrees = Vec::new();

  // Get worktrees
  if let Ok(wt_list) = repo.worktrees() {
    for wt_name in wt_list.iter().flatten() {
      if let Ok(worktree) = repo.find_worktree(wt_name) {
        if let (Some(path), Some(name)) = (worktree.path().to_str(), worktree.name()) {
          worktrees.push(json!({
              "name": name,
              "path": path,
              "is_locked": worktree.is_locked().is_ok(),
          }));
        }
      }
    }
  }

  Ok(CallToolResult {
    content: vec![ToolContent::Text {
      text: serde_json::to_string_pretty(&json!({
          "worktrees": worktrees,
          "total": worktrees.len()
      }))?,
    }],
    is_error: None,
  })
}

async fn get_registry(_args: Option<Value>) -> Result<CallToolResult> {
  let config_dirs = ConfigDirs::new()?;
  let registry = Registry::load(&config_dirs)?;
  let repos: Vec<_> = registry
    .list()
    .iter()
    .map(|repo| {
      json!({
          "name": repo.name,
          "path": repo.path
      })
    })
    .collect();

  Ok(CallToolResult {
    content: vec![ToolContent::Text {
      text: serde_json::to_string_pretty(&json!({
          "repositories": repos,
          "total": repos.len()
      }))?,
    }],
    is_error: None,
  })
}

fn get_repo_path(args: Option<Value>) -> Result<PathBuf> {
  if let Some(args) = args {
    if let Some(path) = args.get("repo_path").and_then(|p| p.as_str()) {
      return Ok(PathBuf::from(path));
    }
  }

  // Default to current directory
  std::env::current_dir().context("Failed to get current directory")
}
