//! # Dashboard Command
//!
//! Provides a comprehensive dashboard view of branches, PRs, and issues.

use anyhow::Result;
use clap::{Arg, Command};
use colored::Colorize;
use git2::{BranchType, Repository as Git2Repository};
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tokio::runtime::Runtime;
use twig_gh::{GitHubPullRequest, create_github_client};
use twig_jira::{Issue, create_jira_client};

use crate::consts::{DEFAULT_JIRA_HOST, ENV_JIRA_HOST};
use crate::creds::{get_github_credentials, get_jira_credentials};
use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::utils::output::{print_error, print_warning};

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

/// Build the dashboard command
pub fn build_command() -> Command {
  Command::new("dashboard")
    .about("Show a comprehensive dashboard of local branches, PRs, and issues")
    .long_about(
      "Show a comprehensive dashboard of local branches, PRs, and issues.\n\n\
             This command displays a unified view of your development context,\n\
             including local branches, associated pull requests, and related Jira issues.\n\
             It helps you keep track of your work across different systems.\n\n\
             By default, only local branches are shown. Use --include-remote to include remote branches.",
    )
    .alias("dash")
    .arg(
      Arg::new("mine")
        .help("Show only items assigned to or created by the current user")
        .long("mine")
        .short('m')
        .action(clap::ArgAction::SetTrue),
    )
    .arg(
      Arg::new("recent")
        .help("Show only recent items (last 7 days)")
        .long("recent")
        .short('r')
        .action(clap::ArgAction::SetTrue),
    )
    .arg(
      Arg::new("repo")
        .help("Path to a specific repository (defaults to current repository)")
        .long("repo")
        .short('p')
        .value_name("PATH"),
    )
    .arg(
      Arg::new("format")
        .help("Output format (text, json)")
        .long("format")
        .short('f')
        .value_name("FORMAT")
        .value_parser(["text", "json"])
        .default_value("text"),
    )
    .arg(
      Arg::new("include_remote")
        .help("Include remote branches in the dashboard")
        .long("include-remote")
        .action(clap::ArgAction::SetTrue),
    )
}

/// Handle the dashboard command
pub fn handle_command(matches: &clap::ArgMatches) -> Result<()> {
  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get command options
  let show_mine_only = matches.get_flag("mine");
  let show_recent_only = matches.get_flag("recent");
  let include_remote = matches.get_flag("include_remote");
  let output_format = matches.get_one::<String>("format").unwrap();

  // Get repository path (current or specified)
  let repo_path = if let Some(path) = matches.get_one::<String>("repo") {
    std::path::PathBuf::from(path)
  } else {
    match detect_current_repository() {
      Ok(path) => path,
      Err(e) => {
        print_error(&format!("Failed to detect current repository: {e}"));
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
  let branch_iter = match repo.branches(if include_remote { None } else { Some(BranchType::Local) }) {
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
    let datetime = chrono::DateTime::from_timestamp(time.seconds(), 0).unwrap();
    let last_commit_date = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

    // Skip if we're only showing recent branches
    if show_recent_only {
      let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
      if datetime < seven_days_ago {
        continue;
      }
    }

    // Get associated PR and Jira issue
    let branch_metadata = repo_state.get_branch_issue_by_branch(&branch_name);
    let github_pr = branch_metadata.as_ref().and_then(|m| m.github_pr);
    let jira_issue = branch_metadata.as_ref().and_then(|m| m.jira_issue.clone());

    branches.push(BranchInfo {
      name: branch_name,
      last_commit_date,
      github_pr,
      jira_issue,
    });
  }

  // Collect GitHub PRs
  let mut pull_requests = Vec::new();
  if let Ok(creds) = get_github_credentials() {
    if let Ok(github_client) = create_github_client(&creds.username, &creds.password) {
      if let Ok((owner, repo_name)) = github_client.extract_repo_info_from_url(remote_url) {
        match rt.block_on(github_client.list_pull_requests(&owner, &repo_name, Some("open"), None)) {
          Ok(prs) => {
            for pr in prs {
              // Skip if we're only showing PRs created by the current user
              if show_mine_only && pr.user.login != creds.username {
                continue;
              }

              // Skip if we're only showing recent PRs
              if show_recent_only {
                if let Ok(created_date) = chrono::DateTime::parse_from_rfc3339(&pr.created_at) {
                  let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
                  if created_date < seven_days_ago {
                    continue;
                  }
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
  }

  // Collect Jira issues
  let mut issues = Vec::new();
  if let Ok(creds) = get_jira_credentials() {
    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

    if let Ok(jira_client) = create_jira_client(&jira_host, &creds.username, &creds.password) {
      // Set up JQL filters
      let assignee = if show_mine_only { Some("me") } else { None };
      let pagination = Some((50, 0));

      match rt.block_on(jira_client.list_issues(None, None, assignee, pagination)) {
        Ok(jira_issues) => {
          for issue in jira_issues {
            // Skip if we're only showing recent issues
            if show_recent_only {
              if let Ok(updated_date) = chrono::DateTime::parse_from_rfc3339(&issue.fields.updated) {
                let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
                if updated_date < seven_days_ago {
                  continue;
                }
              }
            }

            issues.push(issue);
          }
        }
        Err(e) => {
          print_warning(&format!("Failed to fetch Jira issues: {e}"));
        }
      }
    }
  }

  // Create dashboard data
  let dashboard_data = DashboardData {
    branches,
    pull_requests,
    issues,
  };

  // Output the dashboard data
  match output_format.as_str() {
    "json" => {
      // Output as JSON
      match serde_json::to_string_pretty(&dashboard_data) {
        Ok(json) => println!("{json}"),
        Err(e) => print_error(&format!("Failed to serialize dashboard data: {e}")),
      }
    }
    _ => {
      // Output as text
      display_text_dashboard(&dashboard_data, include_remote);
    }
  }

  Ok(())
}

/// Display the dashboard in text format
fn display_text_dashboard(data: &DashboardData, include_remote: bool) {
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
    "\n{}",
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

    // Create and display the table with a simpler style
    println!("{}", Table::new(branch_rows).with(Style::ascii()));
  }

  // Display pull requests
  println!("\n{}", "GitHub Pull Requests".bold().underline());
  if data.pull_requests.is_empty() {
    println!("  No pull requests found");
  } else {
    // Convert PRs to table rows
    let pr_rows: Vec<PullRequestRow> = data
      .pull_requests
      .iter()
      .map(|pr| {
        // Truncate title if too long
        let title = if pr.title.len() > 47 {
          format!("{}...", &pr.title[0..44])
        } else {
          pr.title.clone()
        };

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

    // Create and display the table with a simpler style
    println!("{}", Table::new(pr_rows).with(Style::ascii()));
  }

  // Display Jira issues
  println!("\n{}", "Jira Issues".bold().underline());
  if data.issues.is_empty() {
    println!("  No issues found");
  } else {
    // Convert issues to table rows
    let issue_rows: Vec<IssueRow> = data
      .issues
      .iter()
      .map(|issue| {
        // Truncate summary if too long
        let summary = if issue.fields.summary.len() > 47 {
          format!("{}...", &issue.fields.summary[0..44])
        } else {
          issue.fields.summary.clone()
        };

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

    // Create and display the table with a simpler style
    println!("{}", Table::new(issue_rows).with(Style::ascii()));
  }

  println!();
}
