//! # Dashboard Command
//!
//! Derive-based implementation of the dashboard command for providing a
//! comprehensive view of branches, PRs, and issues.

use anyhow::Result;
use clap::Args;
use directories::BaseDirs;
use git2::{BranchType, Repository as Git2Repository};
use owo_colors::OwoColorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tokio::runtime::Runtime;
use twig_core::output::{print_error, print_warning};
use twig_core::state::RepoState;
use twig_core::truncate_string;
use twig_gh::{GitHubPullRequest, create_github_client, extract_repo_info_from_url, get_github_credentials};
use twig_jira::{Issue, create_jira_client_from_netrc, get_jira_host};

// Structure to hold dashboard data
#[derive(Serialize)]
pub struct DashboardData {
  pub branches: Vec<BranchInfo>,
  pub pull_requests: Vec<GitHubPullRequest>,
  pub issues: Vec<Issue>,
}

#[derive(Serialize)]
pub struct BranchInfo {
  pub name: String,
  pub last_commit_date: String,
  pub github_pr: Option<u32>,
  pub jira_issue: Option<String>,
}

/// Command for showing a comprehensive dashboard
#[derive(Args)]
pub struct DashboardArgs {
  /// Show only items assigned to or created by the current user
  #[arg(long, short = 'm')]
  pub mine: bool,

  /// Show only recent items (last 7 days)
  #[arg(long, short = 'r')]
  pub recent: bool,

  /// Path to a specific repository (defaults to current repository)
  #[arg(long, short = 'p', value_name = "PATH")]
  pub repo: Option<String>,

  /// Output format
  #[arg(long, short = 'f', value_name = "FORMAT", value_parser = ["text", "json"], default_value = "text")]
  pub format: String,

  /// Include remote branches in the dashboard
  #[arg(long = "include-remote")]
  pub include_remote: bool,

  /// Disable GitHub PR information (avoids GitHub API requests)
  #[arg(long = "no-github")]
  pub no_github: bool,

  /// Disable Jira issue information (avoids Jira API requests)
  #[arg(long = "no-jira")]
  pub no_jira: bool,

  /// Simple view (equivalent to --no-github --no-jira)
  #[arg(long, short = 's')]
  pub simple: bool,
}

/// Handle the dashboard command
///
/// This function collects information about branches, GitHub pull requests,
/// and Jira issues, and displays them in a formatted dashboard.
pub(crate) fn handle_dashboard_command(dashboard: DashboardArgs) -> Result<()> {
  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get repository path (current or specified)
  let repo_path = if let Some(path) = dashboard.repo {
    std::path::PathBuf::from(path)
  } else {
    match twig_core::detect_repository() {
      Some(path) => path,
      None => {
        print_error("Failed to detect current repository");
        return Ok(());
      }
    }
  };

  // Open the git repository
  let repo = match Git2Repository::open(&repo_path) {
    Ok(repo) => repo,
    Err(e) => {
      print_error(&format!("Failed to open git repository: {e}"));
      return Ok(());
    }
  };

  // Load the repository state
  let repo_state = match RepoState::load(&repo_path) {
    Ok(state) => state,
    Err(e) => {
      print_error(&format!("Failed to load repository state: {e}"));
      return Ok(());
    }
  };

  // Get the remote URL to extract owner and repo
  let remote = match repo.find_remote("origin") {
    Ok(remote) => remote,
    Err(e) => {
      print_error(&format!("Failed to find remote 'origin': {e}"));
      return Ok(());
    }
  };

  let remote_url = match remote.url() {
    Some(url) => url,
    None => {
      print_error("Failed to get remote URL");
      return Ok(());
    }
  };

  // Collect branch information
  let mut branches = Vec::new();
  let branch_iter = match repo.branches(if dashboard.include_remote {
    None
  } else {
    Some(BranchType::Local)
  }) {
    Ok(branches) => branches,
    Err(e) => {
      print_error(&format!("Failed to list branches: {e}"));
      return Ok(());
    }
  };

  for (branch, _) in branch_iter.flatten() {
    let branch_name = match branch.name() {
      Ok(Some(name)) => name.to_string(),
      _ => continue,
    };

    // Get the last commit date
    let commit = match branch.get().peel_to_commit() {
      Ok(commit) => commit,
      Err(_) => continue,
    };

    let time = commit.time();
    let datetime = chrono::DateTime::from_timestamp(time.seconds(), 0).unwrap_or(chrono::DateTime::UNIX_EPOCH);
    let last_commit_date = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

    // Skip if we're only showing recent branches
    if dashboard.recent {
      let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
      if datetime < seven_days_ago {
        continue;
      }
    }

    // Get associated PR and Jira issue
    let branch_metadata = repo_state.get_branch_metadata(&branch_name);
    let github_pr = branch_metadata.as_ref().and_then(|m| m.github_pr);
    let jira_issue = branch_metadata.as_ref().and_then(|m| m.jira_issue.clone());

    branches.push(BranchInfo {
      name: branch_name,
      last_commit_date,
      github_pr,
      jira_issue,
    });
  }

  // Check if we should skip GitHub and Jira API calls
  let skip_github = dashboard.no_github || dashboard.simple;
  let skip_jira = dashboard.no_jira || dashboard.simple;

  // Collect GitHub PRs
  let mut pull_requests = Vec::new();
  if !skip_github {
    if let Some(base_dirs) = BaseDirs::new() {
      // Get GitHub credentials from .netrc
      if let Ok(creds) = get_github_credentials(base_dirs.home_dir()) {
        let gh = create_github_client(&creds.username, &creds.password);
        if let Ok((owner, repo_name)) = extract_repo_info_from_url(remote_url) {
          match rt.block_on(gh.list_pull_requests(&owner, &repo_name, Some("open"), None)) {
            Ok(prs) => {
              for pr in prs {
                // Skip if we're only showing PRs created by the current user
                if dashboard.mine && pr.user.login != creds.username {
                  continue;
                }

                // Skip if we're only showing recent PRs
                if dashboard.recent
                  && let Ok(created_date) = chrono::DateTime::parse_from_rfc3339(&pr.created_at)
                {
                  let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
                  if created_date < seven_days_ago {
                    continue;
                  }
                }

                pull_requests.push(pr);
              }
            }
            Err(e) => {
              print_warning(&format!("Failed to fetch GitHub pull requests: {e}"));
            }
          }
        }
      }
    } else {
      print_error("Failed to get home directory for .netrc");
    }
  }

  // Collect Jira issues
  let mut issues = Vec::new();
  if !skip_jira {
    if let Some(base_dirs) = BaseDirs::new() {
      // Get Jira credentials from .netrc
      let jira_host = get_jira_host()?;
      let jira = create_jira_client_from_netrc(base_dirs.home_dir(), &jira_host)?;

      // Set up JQL filters
      let assignee = if dashboard.mine { Some("me") } else { None };
      let pagination = Some((50, 0));

      match rt.block_on(jira.list_issues(None, None, assignee, pagination)) {
        Ok(jira_issues) => {
          for issue in jira_issues {
            // Skip if we're only showing recent issues
            if dashboard.recent
              && let Ok(updated_date) = chrono::DateTime::parse_from_rfc3339(&issue.fields.updated)
            {
              let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
              if updated_date < seven_days_ago {
                continue;
              }
            }

            issues.push(issue);
          }
        }
        Err(e) => {
          print_warning(&format!("Failed to fetch Jira issues: {e}"));
        }
      }
    } else {
      print_error("Failed to get home directory for .netrc");
    }
  }

  // Create dashboard data
  let dashboard_data = DashboardData {
    branches,
    pull_requests,
    issues,
  };

  // Output the dashboard data
  match dashboard.format.as_str() {
    "json" => {
      // Output as JSON
      match serde_json::to_string_pretty(&dashboard_data) {
        Ok(json) => println!("{json}"),
        Err(e) => print_error(&format!("Failed to serialize dashboard data: {e}")),
      }
    }
    _ => {
      // Output as text
      let skip_github = dashboard.no_github || dashboard.simple;
      let skip_jira = dashboard.no_jira || dashboard.simple;
      display_text_dashboard(&dashboard_data, dashboard.include_remote, skip_github, skip_jira);
    }
  }

  Ok(())
}

/// Display the dashboard in text format
fn display_text_dashboard(data: &DashboardData, include_remote: bool, skip_github: bool, skip_jira: bool) {
  // Define a struct for branch data with Tabled trait
  #[derive(Tabled)]
  struct BranchRow {
    #[tabled(rename = "Branch Name")]
    name: String,
    #[tabled(rename = "Last Commit")]
    last_commit: String,
    #[tabled(rename = "GitHub PR")]
    github_pr: String,
    #[tabled(rename = "Jira Issue")]
    jira_issue: String,
  }

  // Define a struct for PR data with Tabled trait
  #[derive(Tabled)]
  struct PullRequestRow {
    #[tabled(rename = "PR #")]
    number: u32,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Author")]
    author: String,
    #[tabled(rename = "Created")]
    created: String,
  }

  // Define a struct for issue data with Tabled trait
  #[derive(Tabled)]
  struct IssueRow {
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "Summary")]
    summary: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Assignee")]
    assignee: String,
  }

  // Display branches
  println!(
    "{}",
    (if include_remote {
      "All Branches (Local & Remote)"
    } else {
      "Local Branches"
    })
    .bold()
    .underline()
  );
  if data.branches.is_empty() {
    println!("  No branches found");
  } else {
    // Convert branches to table rows
    let branch_rows: Vec<BranchRow> = data
      .branches
      .iter()
      .map(|branch| {
        let pr_info = match branch.github_pr {
          Some(pr_number) => format!("#{pr_number}"),
          None => "None".into(),
        };

        let issue_info = match &branch.jira_issue {
          Some(issue_key) => issue_key.clone(),
          None => "None".into(),
        };

        BranchRow {
          name: branch.name.clone(),
          last_commit: branch.last_commit_date.clone(),
          github_pr: pr_info,
          jira_issue: issue_info,
        }
      })
      .collect();

    println!("\n{}", Table::new(branch_rows).with(Style::sharp()));
  }

  // Display pull requests if not skipped
  if !skip_github {
    println!("\n{}\n", "GitHub Pull Requests".bold().underline());
    if data.pull_requests.is_empty() {
      println!("  No pull requests found");
    } else {
      // Convert PRs to table rows
      let pr_rows: Vec<PullRequestRow> = data
        .pull_requests
        .iter()
        .map(|pr| {
          // Truncate title if too long (UTF-8 safe)
          let title = truncate_string(&pr.title, 44);

          // Format date to be more readable
          let created_date = pr.created_at.split('T').next().unwrap_or(&pr.created_at);

          PullRequestRow {
            number: pr.number,
            title,
            author: pr.user.login.clone(),
            created: created_date.to_string(),
          }
        })
        .collect();

      println!("{}", Table::new(pr_rows).with(Style::sharp()));
    }
  }

  // Display Jira issues if not skipped
  if !skip_jira {
    println!("\n{}\n", "Jira Issues".bold().underline());
    if data.issues.is_empty() {
      println!("  No issues found");
    } else {
      // Convert issues to table rows
      let issue_rows: Vec<IssueRow> = data
        .issues
        .iter()
        .map(|issue| {
          // Truncate summary if too long (UTF-8 safe)
          let summary = truncate_string(&issue.fields.summary, 44);

          // Get assignee
          let assignee = issue
            .fields
            .assignee
            .as_ref()
            .map(|a| a.display_name.clone())
            .unwrap_or_else(|| "Unassigned".to_string());

          IssueRow {
            key: issue.key.clone(),
            summary,
            status: issue.fields.status.name.clone(),
            assignee,
          }
        })
        .collect();

      println!("{}", Table::new(issue_rows).with(Style::sharp()));
    }
  }

  println!();
}
