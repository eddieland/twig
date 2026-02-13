mod cli;

use std::collections::HashSet;

use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::Confirm;
use git2::BranchType;
use twig_core::GitHubRepo;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::plugin::PluginContext;
use twig_core::state::RepoState;

use crate::cli::Cli;

/// A local branch eligible for pruning.
struct Candidate {
  branch_name: String,
  /// Human-readable reason, e.g. "PR #42 (Add feature)" or "PROJ-123 (Done)".
  description: String,
}

/// Execute the plugin with the provided command-line arguments.
pub fn run() -> Result<()> {
  let cli = Cli::parse();
  let ctx = PluginContext::discover()?;

  let repo_path = ctx
    .current_repo
    .as_deref()
    .ok_or_else(|| anyhow::anyhow!("Not in a git repository"))?;

  let repo = git2::Repository::open(repo_path).context("Failed to open git repository")?;

  let current_branch = ctx.current_branch.clone();

  // Detect GitHub remote
  let github_repo = {
    let remote = repo
      .find_remote("origin")
      .context("No 'origin' remote found. This plugin requires a GitHub remote.")?;
    let remote_url = remote
      .url()
      .ok_or_else(|| anyhow::anyhow!("Remote 'origin' has no URL"))?;
    GitHubRepo::parse(remote_url).context("Could not parse GitHub owner/repo from the origin remote URL")?
  };

  // Load repo state for PR associations
  let state = RepoState::load(repo_path).unwrap_or_default();
  let root_branches: HashSet<String> = state.get_root_branches().into_iter().collect();

  // Collect local branch names eligible for pruning (not current, not root)
  let branches = repo.branches(Some(BranchType::Local))?;
  let mut eligible_branches: Vec<String> = Vec::new();

  for branch_result in branches {
    let (branch, _) = branch_result?;
    let name = match branch.name()? {
      Some(n) => n.to_string(),
      None => continue,
    };

    // Never prune the current branch or root branches
    if current_branch.as_deref() == Some(name.as_str()) || root_branches.contains(&name) {
      continue;
    }

    eligible_branches.push(name);
  }

  // Partition into branches with PRs
  let branches_with_prs: Vec<(String, u32)> = eligible_branches
    .iter()
    .filter_map(|name| {
      state
        .get_branch_metadata(name)
        .and_then(|m| m.github_pr)
        .map(|pr| (name.clone(), pr))
    })
    .collect();

  let home = directories::BaseDirs::new().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
  let mut candidates: Vec<Candidate> = Vec::new();

  // --- GitHub PR check ---
  if !branches_with_prs.is_empty() {
    match twig_gh::create_github_runtime_and_client(home.home_dir()) {
      Ok((rt, gh)) => {
        print_info(&format!(
          "Checking {} PR(s) for {}",
          branches_with_prs.len(),
          github_repo.full_name()
        ));

        for (branch_name, pr_number) in &branches_with_prs {
          match rt.block_on(gh.get_pull_request(&github_repo.owner, &github_repo.repo, *pr_number)) {
            Ok(pr) if pr.merged_at.is_some() => {
              candidates.push(Candidate {
                branch_name: branch_name.clone(),
                description: format!("PR #{} ({})", pr.number, pr.title),
              });
            }
            Ok(_) => {} // PR exists but not merged
            Err(e) => {
              print_warning(&format!("Could not fetch PR #{pr_number} for '{branch_name}': {e}"));
            }
          }
        }
      }
      Err(e) => {
        print_warning(&format!("Could not create GitHub client, skipping PR checks: {e}"));
      }
    }
  }

  // --- Jira issue check ---
  let matched: HashSet<&str> = candidates.iter().map(|c| c.branch_name.as_str()).collect();

  let branches_with_jira: Vec<(String, String)> = eligible_branches
    .iter()
    .filter(|name| !matched.contains(name.as_str()))
    .filter_map(|name| {
      state
        .get_branch_metadata(name)
        .and_then(|m| m.jira_issue.clone())
        .map(|issue| (name.clone(), issue))
    })
    .collect();

  if !branches_with_jira.is_empty()
    && let Ok(jira_host) = twig_jira::get_jira_host()
    && let Ok((jira_rt, jira)) = twig_jira::create_jira_runtime_and_client(home.home_dir(), &jira_host)
  {
    const DONE_STATUSES: &[&str] = &["done", "closed", "resolved"];

    print_info(&format!("Checking {} Jira issue(s)", branches_with_jira.len()));

    for (branch_name, issue_key) in &branches_with_jira {
      match jira_rt.block_on(jira.get_issue(issue_key)) {
        Ok(issue) => {
          let status = issue.fields.status.name.to_lowercase();
          if DONE_STATUSES.contains(&status.as_str()) {
            candidates.push(Candidate {
              branch_name: branch_name.clone(),
              description: format!("{} ({})", issue_key, issue.fields.status.name),
            });
          }
        }
        Err(e) => {
          print_warning(&format!(
            "Could not fetch Jira issue {issue_key} for '{branch_name}': {e}"
          ));
        }
      }
    }
  }

  if candidates.is_empty() {
    print_info("No local branches with merged PRs or done Jira issues found.");
    return Ok(());
  }

  candidates.sort_by(|a, b| a.branch_name.cmp(&b.branch_name));

  print_info(&format!("Found {} branch(es) to prune:\n", candidates.len()));

  if cli.dry_run {
    for candidate in &candidates {
      println!("  {} \u{2014} {}", candidate.branch_name, candidate.description);
    }
    println!();
    print_info("Dry run \u{2014} no branches were deleted.");
    return Ok(());
  }

  let mut deleted_count: u32 = 0;
  let mut skipped_count: u32 = 0;

  for candidate in &candidates {
    println!("  {} \u{2014} {}", candidate.branch_name, candidate.description);

    let should_delete = if cli.skip_prompts {
      true
    } else {
      Confirm::new()
        .with_prompt(format!("  Delete '{}'?", candidate.branch_name))
        .default(false)
        .interact()
        .unwrap_or(false)
    };

    if should_delete {
      match delete_local_branch(&repo, &candidate.branch_name) {
        Ok(()) => {
          print_success(&format!("  Deleted {}", candidate.branch_name));
          deleted_count += 1;
        }
        Err(e) => {
          print_error(&format!("  Failed to delete {}: {}", candidate.branch_name, e));
        }
      }
    } else {
      skipped_count += 1;
    }
  }

  // Clean up twig state for any deleted branches
  if deleted_count > 0 {
    let local_branches: HashSet<String> = repo
      .branches(Some(BranchType::Local))
      .into_iter()
      .flatten()
      .filter_map(|b| b.ok())
      .filter_map(|(b, _)| b.name().ok().flatten().map(|n| n.to_string()))
      .collect();

    let mut state = RepoState::load(repo_path).unwrap_or_default();
    state.evict_stale_branches(&local_branches);
    if let Err(e) = state.save(repo_path) {
      print_error(&format!("Failed to update twig state: {e}"));
    }
  }

  // Summary
  println!();
  if deleted_count > 0 {
    print_success(&format!("Pruned {deleted_count} branch(es)"));
  }
  if skipped_count > 0 {
    print_info(&format!("Skipped {skipped_count} branch(es)"));
  }

  Ok(())
}

fn delete_local_branch(repo: &git2::Repository, branch_name: &str) -> Result<()> {
  let mut branch = repo
    .find_branch(branch_name, BranchType::Local)
    .with_context(|| format!("Branch '{branch_name}' not found"))?;
  branch
    .delete()
    .with_context(|| format!("Failed to delete branch '{branch_name}'"))?;
  Ok(())
}
