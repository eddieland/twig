mod cli;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::Confirm;
use git2::BranchType;
use twig_core::output::{print_error, print_info, print_success};
use twig_core::plugin::PluginContext;
use twig_core::state::RepoState;
use twig_core::GitHubRepo;
use twig_gh::endpoints::pulls::PaginationOptions;
use twig_gh::GitHubPullRequest;

use crate::cli::Cli;

/// A local branch matched to a merged PR.
struct Candidate {
  branch_name: String,
  pr_number: u32,
  pr_title: String,
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

  // Create GitHub client
  let home =
    directories::BaseDirs::new().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
  let (rt, gh) = twig_gh::create_github_runtime_and_client(home.home_dir())?;

  // Fetch recently merged PRs
  print_info(&format!(
    "Checking merged PRs for {}",
    github_repo.full_name()
  ));

  let merged_prs: Vec<GitHubPullRequest> = rt.block_on(async {
    let pagination = PaginationOptions {
      per_page: 100,
      page: 1,
    };
    let prs = gh
      .list_pull_requests(&github_repo.owner, &github_repo.repo, Some("closed"), Some(pagination))
      .await?;

    Ok::<_, anyhow::Error>(
      prs
        .into_iter()
        .filter(|pr| pr.merged_at.is_some())
        .collect(),
    )
  })?;

  if merged_prs.is_empty() {
    print_info("No recently merged PRs found.");
    return Ok(());
  }

  // Build lookups: head branch name -> PR, PR number -> PR
  let mut branch_to_pr: HashMap<&str, &GitHubPullRequest> = HashMap::new();
  let mut pr_number_to_pr: HashMap<u32, &GitHubPullRequest> = HashMap::new();
  for pr in &merged_prs {
    if let Some(ref_name) = &pr.head.ref_name {
      branch_to_pr.insert(ref_name.as_str(), pr);
    }
    pr_number_to_pr.insert(pr.number, pr);
  }

  // Load repo state for additional PR associations
  let state = RepoState::load(repo_path).unwrap_or_default();
  let root_branches: HashSet<String> = state.get_root_branches().into_iter().collect();

  // Match local branches to merged PRs
  let branches = repo.branches(Some(BranchType::Local))?;
  let mut candidates: Vec<Candidate> = Vec::new();
  let mut seen: HashSet<String> = HashSet::new();

  for branch_result in branches {
    let (branch, _) = branch_result?;
    let name = match branch.name()? {
      Some(n) => n.to_string(),
      None => continue,
    };

    // Never prune the current branch
    if current_branch.as_deref() == Some(name.as_str()) {
      continue;
    }

    // Never prune root branches
    if root_branches.contains(&name) {
      continue;
    }

    // Match by head branch name
    if let Some(pr) = branch_to_pr.get(name.as_str()) {
      seen.insert(name.clone());
      candidates.push(Candidate {
        branch_name: name,
        pr_number: pr.number,
        pr_title: pr.title.clone(),
      });
      continue;
    }

    // Match by PR number from twig state
    if let Some(metadata) = state.get_branch_metadata(&name)
      && let Some(pr_number) = metadata.github_pr
      && let Some(pr) = pr_number_to_pr.get(&pr_number)
      && !seen.contains(&name)
    {
      seen.insert(name.clone());
      candidates.push(Candidate {
        branch_name: name,
        pr_number: pr.number,
        pr_title: pr.title.clone(),
      });
    }
  }

  if candidates.is_empty() {
    print_info("No local branches with merged PRs found.");
    return Ok(());
  }

  candidates.sort_by(|a, b| a.branch_name.cmp(&b.branch_name));

  print_info(&format!(
    "Found {} branch(es) with merged PRs:\n",
    candidates.len()
  ));

  if cli.dry_run {
    for candidate in &candidates {
      println!(
        "  {} \u{2014} PR #{} ({})",
        candidate.branch_name, candidate.pr_number, candidate.pr_title
      );
    }
    println!();
    print_info("Dry run \u{2014} no branches were deleted.");
    return Ok(());
  }

  let mut deleted_count: u32 = 0;
  let mut skipped_count: u32 = 0;

  for candidate in &candidates {
    println!(
      "  {} \u{2014} PR #{} ({})",
      candidate.branch_name, candidate.pr_number, candidate.pr_title
    );

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
          print_error(&format!(
            "  Failed to delete {}: {}",
            candidate.branch_name, e
          ));
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
