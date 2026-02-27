mod cli;

use std::collections::HashSet;
use std::fmt;

use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::MultiSelect;
use git2::BranchType;
use owo_colors::OwoColorize;
use twig_core::git::delete_local_branch;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::plugin::PluginContext;
use twig_core::state::RepoState;
use twig_core::{GitHubRepo, twig_theme};

use crate::cli::Cli;

/// Why a branch is eligible for pruning.
enum PruneReason {
  /// Associated GitHub PR was merged.
  MergedPr { number: u32, title: String },
  /// Associated Jira issue reached a done status.
  JiraDone { key: String, status: String },
}

/// A local branch eligible for pruning.
struct Candidate {
  branch_name: String,
  reason: PruneReason,
}

impl Candidate {
  /// Short label shown in the multi-select list.
  fn select_label(&self) -> String {
    match &self.reason {
      PruneReason::MergedPr { number, title } => {
        format!("{} — 🔀 PR #{number} ({title})", self.branch_name)
      }
      PruneReason::JiraDone { key, status } => {
        format!("{} — 🎫 {key} ({status})", self.branch_name)
      }
    }
  }
}

impl fmt::Display for Candidate {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.select_label())
  }
}

/// Tracks results across the prune operation.
#[derive(Default)]
struct PruneSummary {
  total_candidates: usize,
  deleted: Vec<String>,
  skipped: Vec<String>,
  errors: Vec<(String, String)>,
}

fn pluralize(count: usize, singular: &str, plural: &str) -> String {
  if count == 1 {
    format!("{count} {singular}")
  } else {
    format!("{count} {plural}")
  }
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

  if eligible_branches.is_empty() {
    print_info("No eligible local branches found (all are root or current).");
    return Ok(());
  }

  println!(
    "{}",
    format!("Scanning {} for prunable branches...", eligible_branches.len()).dimmed()
  );

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
          "Checking {} for {}",
          pluralize(branches_with_prs.len(), "PR", "PRs"),
          github_repo.full_name()
        ));

        for (branch_name, pr_number) in &branches_with_prs {
          match rt.block_on(gh.get_pull_request(&github_repo.owner, &github_repo.repo, *pr_number)) {
            Ok(pr) if pr.merged_at.is_some() => {
              candidates.push(Candidate {
                branch_name: branch_name.clone(),
                reason: PruneReason::MergedPr {
                  number: pr.number,
                  title: pr.title.clone(),
                },
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

  if !branches_with_jira.is_empty() {
    if let Ok(jira_host) = twig_jira::get_jira_host()
      && let Ok((jira_rt, jira)) = twig_jira::create_jira_runtime_and_client(home.home_dir(), &jira_host)
    {
      const DONE_STATUSES: &[&str] = &["done", "closed", "resolved"];

      print_info(&format!(
        "Checking {}",
        pluralize(branches_with_jira.len(), "Jira issue", "Jira issues"),
      ));

      for (branch_name, issue_key) in &branches_with_jira {
        match jira_rt.block_on(jira.get_issue(issue_key)) {
          Ok(issue) => {
            let status = issue.fields.status.name.to_lowercase();
            if DONE_STATUSES.contains(&status.as_str()) {
              candidates.push(Candidate {
                branch_name: branch_name.clone(),
                reason: PruneReason::JiraDone {
                  key: issue_key.clone(),
                  status: issue.fields.status.name.clone(),
                },
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
    } else {
      print_warning(&format!(
        "Skipping {} (JIRA_HOST not set or credentials unavailable).",
        pluralize(
          branches_with_jira.len(),
          "branch with Jira issue",
          "branches with Jira issues"
        ),
      ));
    }
  }

  if candidates.is_empty() {
    print_info("No local branches with merged PRs or done Jira issues found.");
    return Ok(());
  }

  candidates.sort_by(|a, b| a.branch_name.cmp(&b.branch_name));

  // --- Display candidates ---
  println!();
  print_success(&format!(
    "Found {}:",
    pluralize(candidates.len(), "branch to prune", "branches to prune"),
  ));
  println!();

  for (i, candidate) in candidates.iter().enumerate() {
    display_candidate(candidate, i + 1, candidates.len());
  }

  // --- Dry-run: just list and exit ---
  if cli.dry_run {
    println!();
    print_info("Dry run — no branches were deleted.");
    return Ok(());
  }

  // --- Deletion ---
  let selected_indices = if cli.skip_prompts {
    // Select all
    (0..candidates.len()).collect::<Vec<_>>()
  } else {
    // Multi-select prompt
    println!();
    let labels: Vec<String> = candidates.iter().map(|c| c.select_label()).collect();

    match MultiSelect::with_theme(&twig_theme())
      .with_prompt("Select branches to delete (space to toggle, enter to confirm)")
      .items(&labels)
      .defaults(&vec![true; labels.len()])
      .interact_opt()
    {
      Ok(Some(s)) => s,
      Ok(None) => {
        print_info("Aborted — no branches were deleted.");
        return Ok(());
      }
      Err(e) => {
        return Err(anyhow::anyhow!("Failed to display selection prompt: {e}"));
      }
    }
  };

  let selected_set: HashSet<usize> = selected_indices.iter().copied().collect();

  let mut summary = PruneSummary {
    total_candidates: candidates.len(),
    ..Default::default()
  };

  println!();

  for (i, candidate) in candidates.iter().enumerate() {
    if !selected_set.contains(&i) {
      summary.skipped.push(candidate.branch_name.clone());
      continue;
    }

    match delete_local_branch(&repo, &candidate.branch_name) {
      Ok(()) => {
        print_success(&format!("Deleted {}", candidate.branch_name.cyan()));
        summary.deleted.push(candidate.branch_name.clone());
      }
      Err(e) => {
        print_error(&format!("Failed to delete {}: {e}", candidate.branch_name));
        summary.errors.push((candidate.branch_name.clone(), e.to_string()));
      }
    }
  }

  // Clean up twig state for any deleted branches
  if !summary.deleted.is_empty() {
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

  // --- Summary ---
  display_summary(&summary);

  Ok(())
}

/// Display a single candidate with rich formatting and a progress divider.
fn display_candidate(candidate: &Candidate, current: usize, total: usize) {
  let separator = "─".repeat(22);
  println!(
    "{} [{}/{}] {}",
    separator.dimmed(),
    current.to_string().dimmed(),
    total.to_string().dimmed(),
    separator.dimmed()
  );

  println!("🌿 Branch: {}", candidate.branch_name.cyan().bold());

  match &candidate.reason {
    PruneReason::MergedPr { number, title } => {
      println!(
        "🔀 PR:     #{} {}",
        number.to_string().yellow(),
        format!("({title})").dimmed(),
      );
    }
    PruneReason::JiraDone { key, status } => {
      println!("🎫 Jira:   {} {}", key.yellow(), format!("({status})").dimmed(),);
    }
  }
  println!();
}

/// Display the final prune summary.
fn display_summary(summary: &PruneSummary) {
  println!();
  println!("{}", "Prune Summary".bold());
  println!("  • Candidates:  {}", summary.total_candidates);

  if !summary.deleted.is_empty() {
    println!(
      "  {} {}     ({})",
      "• Deleted:".green(),
      summary.deleted.len(),
      summary.deleted.join(", "),
    );
  }

  if !summary.skipped.is_empty() {
    println!("  {} {}", "• Skipped:".yellow(), summary.skipped.len(),);
  }

  if !summary.errors.is_empty() {
    println!(
      "  {} {}     ({})",
      "• Errors:".red(),
      summary.errors.len(),
      summary
        .errors
        .iter()
        .map(|(branch, _)| branch.as_str())
        .collect::<Vec<_>>()
        .join(", "),
    );
  }
}
