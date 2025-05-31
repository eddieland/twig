//! # GitHub Command
//!
//! Derive-based implementation of the GitHub command for GitHub integration,
//! including pull request management, status checks, and synchronization with
//! branch metadata for development workflows.

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use git2::Repository as Git2Repository;
use owo_colors::OwoColorize;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tokio::runtime::Runtime;
use twig_gh::{PullRequestStatus, create_github_client};

use crate::cli::derive::DeriveCommand;
use crate::creds::get_github_credentials;
use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::utils::output::{
  format_check_status, format_command, format_pr_review_status, print_error, print_info, print_success, print_warning,
};

/// Command for GitHub integration
#[derive(Parser)]
#[command(name = "github")]
#[command(about = "GitHub integration")]
#[command(long_about = "Interact with GitHub repositories and pull requests.\n\n\
            This command group provides functionality for working with GitHub,\n\
            including checking authentication, viewing pull request status,\n\
            and linking branches to pull requests.")]
#[command(alias = "gh")]
pub struct GitHubCommand {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: GitHubSubcommands,
}

/// Subcommands for the GitHub command
#[derive(Subcommand)]
pub enum GitHubSubcommands {
  /// Check GitHub authentication
  #[command(long_about = "Verify that your GitHub credentials are working correctly.\n\n\
                   This command attempts to authenticate with GitHub using your\n\
                   credentials from .netrc and displays information about the\n\
                   authenticated user if successful.")]
  Check,

  /// View CI/CD checks for a PR
  #[command(long_about = "View CI/CD checks for a GitHub pull request.\n\n\
                      This command displays the status of CI/CD checks for a specific pull request,\n\
                      including check name, status, conclusion, and links to detailed results.")]
  #[command(alias = "ci")]
  Checks(ChecksCommand),

  /// Pull request operations
  #[command(long_about = "Manage GitHub pull requests.\n\n\
                      This command group provides functionality for working with GitHub pull requests,\n\
                      including viewing status and linking branches to pull requests.")]
  Pr(PrCommand),
}

/// View CI/CD checks for a PR
#[derive(Parser)]
pub struct ChecksCommand {
  /// PR number (defaults to current branch's PR)
  #[arg(index = 1)]
  pub pr_number: Option<String>,

  /// Path to a specific repository (defaults to current repository)
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,
}

/// Pull request operations
#[derive(Parser)]
pub struct PrCommand {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: PrSubcommands,
}

/// Subcommands for the PR command
#[derive(Subcommand)]
pub enum PrSubcommands {
  /// List pull requests for a repository
  #[command(long_about = "List pull requests for a repository with filtering options.\n\n\
                           This command displays a table of pull requests with key information\n\
                           such as PR number, title, author, state, and creation date.")]
  #[command(alias = "ls")]
  List(ListCommand),

  /// Show PR status for current branch
  #[command(
    long_about = "Show the status of pull requests associated with the current branch.\n\n\
                           This command displays information about any pull requests that are\n\
                           associated with the current branch, including review status and check results."
  )]
  #[command(alias = "st")]
  Status,

  /// Link a PR to the current branch
  #[command(long_about = "Link a GitHub pull request to the current branch.\n\n\
                           This command associates a GitHub pull request with the current branch,\n\
                           allowing you to easily check its status later.")]
  Link(LinkCommand),
}

/// List pull requests for a repository
#[derive(Parser)]
pub struct ListCommand {
  /// Filter by PR state (open, closed, all)
  #[arg(long, short = 's', value_name = "STATE", default_value = "open")]
  pub state: String,

  /// Path to a specific repository (defaults to current repository)
  #[arg(long, short = 'r', value_name = "PATH")]
  pub repo: Option<String>,

  /// Maximum number of PRs to display
  #[arg(long, short = 'l', value_name = "COUNT", default_value = "30")]
  pub limit: u32,
}

/// Link a PR to the current branch
#[derive(Parser)]
pub struct LinkCommand {
  /// URL or ID of the pull request to link (e.g., 'https://github.com/owner/repo/pull/123' or '123')
  #[arg(required = true, index = 1)]
  pub pr_url_or_id: String,
}

impl GitHubCommand {
  /// Creates a clap Command for this command (for backward compatibility)
  pub fn command() -> clap::Command {
    <Self as CommandFactory>::command()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
      Some(("check", _)) => {
        let cmd = Self {
          subcommand: GitHubSubcommands::Check,
        };
        cmd.execute()
      }
      Some(("checks", checks_matches)) => {
        let pr_number = checks_matches.get_one::<String>("pr_number").cloned();
        let repo = checks_matches.get_one::<String>("repo").cloned();

        let cmd = Self {
          subcommand: GitHubSubcommands::Checks(ChecksCommand { pr_number, repo }),
        };
        cmd.execute()
      }
      Some(("pr", pr_matches)) => match pr_matches.subcommand() {
        Some(("status", _)) => {
          let cmd = Self {
            subcommand: GitHubSubcommands::Pr(PrCommand {
              subcommand: PrSubcommands::Status,
            }),
          };
          cmd.execute()
        }
        Some(("list", list_matches)) => {
          let state = list_matches.get_one::<String>("state").unwrap().clone();
          let repo = list_matches.get_one::<String>("repo").cloned();
          let limit = *list_matches.get_one::<u32>("limit").unwrap();

          let cmd = Self {
            subcommand: GitHubSubcommands::Pr(PrCommand {
              subcommand: PrSubcommands::List(ListCommand { state, repo, limit }),
            }),
          };
          cmd.execute()
        }
        Some(("link", link_matches)) => {
          let pr_url_or_id = link_matches.get_one::<String>("pr_url_or_id").unwrap().clone();

          let cmd = Self {
            subcommand: GitHubSubcommands::Pr(PrCommand {
              subcommand: PrSubcommands::Link(LinkCommand { pr_url_or_id }),
            }),
          };
          cmd.execute()
        }
        _ => {
          print_error("Unknown pr command");
          Ok(())
        }
      },
      _ => {
        print_error("Unknown github command");
        Ok(())
      }
    }
  }
}

impl DeriveCommand for GitHubCommand {
  fn execute(self) -> Result<()> {
    match self.subcommand {
      GitHubSubcommands::Check => handle_check_command(),
      GitHubSubcommands::Checks(cmd) => handle_checks_command(&cmd),
      GitHubSubcommands::Pr(pr_cmd) => match pr_cmd.subcommand {
        PrSubcommands::Status => handle_pr_status_command(),
        PrSubcommands::List(cmd) => handle_pr_list_command(&cmd),
        PrSubcommands::Link(cmd) => handle_pr_link_command(&cmd.pr_url_or_id),
      },
    }
  }
}

/// Handle the check command
fn handle_check_command() -> Result<()> {
  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get GitHub credentials
  let credentials = match get_github_credentials() {
    Ok(creds) => creds,
    Err(e) => {
      print_error(&format!("Failed to get GitHub credentials: {e}"));
      return Ok(());
    }
  };

  // Create GitHub client
  let github_client = create_github_client(&credentials.username, &credentials.password)?;

  // Test connection
  match rt.block_on(github_client.test_connection()) {
    Ok(true) => {
      print_success("Successfully authenticated with GitHub");

      // Get user information
      match rt.block_on(github_client.get_current_user()) {
        Ok(user) => {
          print_info("GitHub User Information:");
          println!("  Username: {}", user.login);
          if let Some(name) = user.name {
            println!("  Name: {name}");
          }
          println!("  User ID: {}", user.id);
        }
        Err(e) => {
          print_error(&format!("Failed to get user information: {e}"));
        }
      }
    }
    Ok(false) => {
      print_error("Authentication failed but no error was returned");
    }
    Err(e) => {
      print_error(&format!("Failed to authenticate with GitHub: {e}"));
    }
  }

  Ok(())
}

/// Handle the checks command
fn handle_checks_command(cmd: &ChecksCommand) -> Result<()> {
  use std::path::PathBuf;

  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get GitHub credentials
  let credentials = match get_github_credentials() {
    Ok(creds) => creds,
    Err(e) => {
      print_error(&format!("Failed to get GitHub credentials: {e}"));
      return Ok(());
    }
  };

  // Create GitHub client
  let github_client = create_github_client(&credentials.username, &credentials.password)?;

  // Get repository path (current or specified)
  let repo_path = if let Some(path) = &cmd.repo {
    PathBuf::from(path)
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

  // Extract owner and repo from remote URL
  let (owner, repo_name) = match github_client.extract_repo_info_from_url(remote_url) {
    Ok((owner, repo)) => (owner, repo),
    Err(e) => {
      print_error(&format!("Failed to extract repository info from URL: {e}"));
      return Ok(());
    }
  };

  // Determine PR number
  let pr_number = if let Some(pr_num_str) = &cmd.pr_number {
    // PR number provided as argument
    match pr_num_str.parse::<u32>() {
      Ok(num) => num,
      Err(_) => {
        print_error(&format!("Invalid PR number: {pr_num_str}"));
        return Ok(());
      }
    }
  } else {
    // Try to get PR number from current branch
    let head = match repo.head() {
      Ok(head) => head,
      Err(e) => {
        print_error(&format!("Failed to get repository HEAD: {e}"));
        return Ok(());
      }
    };

    let branch_name = match head.shorthand() {
      Some(name) => name,
      None => {
        print_error("Failed to get branch name");
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

    // Check if the branch has an associated PR
    let branch_issue = repo_state.get_branch_issue_by_branch(branch_name);

    if let Some(branch_issue) = branch_issue {
      if let Some(pr_number) = branch_issue.github_pr {
        pr_number
      } else {
        print_error(&format!("Branch '{branch_name}' has no associated PR"));
        print_info(&format!(
          "Link a PR with {} or specify a PR number",
          format_command("twig github pr link <pr-url>")
        ));
        return Ok(());
      }
    } else {
      print_error(&format!("Branch '{branch_name}' has no associated PR"));
      print_info(&format!(
        "Link a PR with {} or specify a PR number",
        format_command("twig github pr link <pr-url>")
      ));
      return Ok(());
    }
  };

  // Fetch PR to get the commit SHA
  println!("Fetching PR #{pr_number} for {owner}/{repo_name}...");

  let pr = match rt.block_on(github_client.get_pull_request(&owner, &repo_name, pr_number)) {
    Ok(pr) => pr,
    Err(e) => {
      print_error(&format!("Failed to fetch PR: {e}"));
      return Ok(());
    }
  };

  // Fetch check runs for the PR's head commit
  println!("Fetching checks for commit {}...", pr.head.sha);

  match rt.block_on(github_client.get_check_runs(&owner, &repo_name, &pr.head.sha)) {
    Ok(check_runs) => {
      if check_runs.is_empty() {
        println!("No checks found for this PR");
        return Ok(());
      }

      // Define a struct for check run data with Tabled trait
      #[derive(Tabled)]
      struct CheckRunRow {
        #[tabled(rename = "Check Name")]
        name: String,
        #[tabled(rename = "Status")]
        status: String,
        #[tabled(rename = "Conclusion")]
        conclusion: String,
        #[tabled(rename = "Started At")]
        started_at: String,
      }

      // Convert check runs to table rows
      let rows: Vec<CheckRunRow> = check_runs
        .iter()
        .map(|check| {
          // Format status with color
          let status_colored = match check.status.as_str() {
            "completed" => check.status.green().to_string(),
            "in_progress" => check.status.yellow().to_string(),
            "queued" => check.status.blue().to_string(),
            _ => check.status.default_color().to_string(),
          };

          // Format conclusion with color
          let conclusion = match &check.conclusion {
            Some(conclusion) => match conclusion.as_str() {
              "success" => conclusion.green().to_string(),
              "failure" => conclusion.red().to_string(),
              "neutral" => conclusion.default_color().to_string(),
              "cancelled" => conclusion.yellow().to_string(),
              "skipped" => conclusion.blue().to_string(),
              "timed_out" => conclusion.red().to_string(),
              "action_required" => conclusion.yellow().to_string(),
              _ => conclusion.default_color().to_string(),
            },
            None => "N/A".default_color().to_string(),
          };

          // Format date to be more readable
          let started_date = check.started_at.split('T').next().unwrap_or(&check.started_at);

          CheckRunRow {
            name: check.name.clone(),
            status: status_colored,
            conclusion,
            started_at: started_date.to_string(),
          }
        })
        .collect();

      println!("\n{}", Table::new(rows).with(Style::sharp()));

      // Display details URLs
      println!("\nDetails:");
      for check in &check_runs {
        if let Some(url) = &check.details_url {
          println!("  • {}: {}", check.name, url);
        }
      }
      println!();
    }
    Err(e) => {
      print_error(&format!("Failed to fetch check runs: {e}"));
    }
  }

  Ok(())
}

/// Handle the PR status command
fn handle_pr_status_command() -> Result<()> {
  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get GitHub credentials
  let credentials = match get_github_credentials() {
    Ok(creds) => creds,
    Err(e) => {
      print_error(&format!("Failed to get GitHub credentials: {e}"));
      return Ok(());
    }
  };

  // Create GitHub client
  let github_client = create_github_client(&credentials.username, &credentials.password)?;

  // Get the current repository
  let repo_path = match detect_current_repository() {
    Ok(path) => path,
    Err(e) => {
      print_error(&format!("Failed to detect current repository: {e}"));
      return Ok(());
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

  // Get the current branch
  let head = match repo.head() {
    Ok(head) => head,
    Err(e) => {
      print_error(&format!("Failed to get repository HEAD: {e}"));
      return Ok(());
    }
  };

  let branch_name = match head.shorthand() {
    Some(name) => name,
    None => {
      print_error("Failed to get branch name");
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

  // Check if the branch has an associated PR
  let branch_issue = repo_state.get_branch_issue_by_branch(branch_name);

  if let Some(branch_issue) = branch_issue {
    if let Some(pr_number) = branch_issue.github_pr {
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

      // Extract owner and repo from remote URL
      let (owner, repo_name) = match github_client.extract_repo_info_from_url(remote_url) {
        Ok((owner, repo)) => (owner, repo),
        Err(e) => {
          print_error(&format!("Failed to extract repository info from URL: {e}"));
          return Ok(());
        }
      };

      // Get PR status
      print_info(&format!("Fetching PR status for #{pr_number}..."));

      match rt.block_on(github_client.get_pr_status(&owner, &repo_name, pr_number)) {
        Ok(status) => {
          display_pr_status(&status);
        }
        Err(e) => {
          print_error(&format!("Failed to get PR status: {e}"));
        }
      }
    } else {
      print_warning(&format!("Branch '{branch_name}' has no associated PR"));
      print_info(&format!(
        "Link a PR with {}",
        format_command("twig github pr link <pr-url>")
      ));
    }
  } else {
    print_warning(&format!("Branch '{branch_name}' has no associated PR"));
    print_info(&format!(
      "Link a PR with {}",
      format_command("twig github pr link <pr-url>")
    ));
  }

  Ok(())
}

/// Handle the PR list command
fn handle_pr_list_command(cmd: &ListCommand) -> Result<()> {
  use std::path::PathBuf;

  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get GitHub credentials
  let credentials = match get_github_credentials() {
    Ok(creds) => creds,
    Err(e) => {
      print_error(&format!("Failed to get GitHub credentials: {e}"));
      return Ok(());
    }
  };

  // Create GitHub client
  let github_client = create_github_client(&credentials.username, &credentials.password)?;

  // Get repository path (current or specified)
  let repo_path = if let Some(path) = &cmd.repo {
    PathBuf::from(path)
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

  // Extract owner and repo from remote URL
  let (owner, repo_name) = match github_client.extract_repo_info_from_url(remote_url) {
    Ok((owner, repo)) => (owner, repo),
    Err(e) => {
      print_error(&format!("Failed to extract repository info from URL: {e}"));
      return Ok(());
    }
  };

  let pagination = twig_gh::endpoints::pulls::PaginationOptions {
    per_page: cmd.limit,
    page: 1,
  };

  println!("Fetching {} pull requests for {owner}/{repo_name}...", cmd.state);
  match rt.block_on(github_client.list_pull_requests(&owner, &repo_name, Some(&cmd.state), Some(pagination))) {
    Ok(prs) => {
      if prs.is_empty() {
        println!("No {} pull requests found for {owner}/{repo_name}", cmd.state);
        return Ok(());
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
        #[tabled(rename = "State")]
        state: String,
        #[tabled(rename = "Created")]
        created: String,
      }

      // Convert PRs to table rows
      let rows: Vec<PullRequestRow> = prs
        .into_iter()
        .map(|pr| {
          // Truncate title if too long
          let title = if pr.title.len() > 47 {
            format!("{}...", &pr.title[0..44])
          } else {
            pr.title.clone()
          };

          // Format state with color
          let state_colored = match pr.state.as_str() {
            "open" => pr.state.green().to_string(),
            "closed" => pr.state.red().to_string(),
            _ => pr.state.default_color().to_string(),
          };

          // Format date to be more readable
          let created_date = pr.created_at.split('T').next().unwrap_or(&pr.created_at);

          PullRequestRow {
            number: pr.number,
            title,
            author: pr.user.login,
            state: state_colored,
            created: created_date.to_string(),
          }
        })
        .collect();

      println!("\n{}\n", Table::new(rows).with(Style::sharp()));
    }
    Err(e) => {
      print_error(&format!("Failed to fetch pull requests: {e}"));
    }
  }

  Ok(())
}

/// Handle the PR link command
fn handle_pr_link_command(pr_url_or_id: &str) -> Result<()> {
  // Create a runtime for async operations
  let rt = Runtime::new()?;

  // Get GitHub credentials
  let credentials = match get_github_credentials() {
    Ok(creds) => creds,
    Err(e) => {
      print_error(&format!("Failed to get GitHub credentials: {e}"));
      return Ok(());
    }
  };

  // Create GitHub client
  let github_client = create_github_client(&credentials.username, &credentials.password)?;

  // Get the current repository
  let repo_path = match detect_current_repository() {
    Ok(path) => path,
    Err(e) => {
      print_error(&format!("Failed to detect current repository: {e}"));
      return Ok(());
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

  // Extract owner and repo from remote URL
  let (owner, repo_name) = match github_client.extract_repo_info_from_url(remote_url) {
    Ok((owner, repo)) => (owner, repo),
    Err(e) => {
      print_error(&format!("Failed to extract repository info from URL: {e}"));
      return Ok(());
    }
  };

  // Determine if input is a PR URL or PR ID
  let pr_number = if pr_url_or_id.contains("github.com") && pr_url_or_id.contains("/pull/") {
    // Input is a URL
    match github_client.extract_pr_number_from_url(pr_url_or_id) {
      Ok(number) => number,
      Err(e) => {
        print_error(&format!("Invalid PR URL: {e}"));
        return Ok(());
      }
    }
  } else {
    // Input is a PR ID
    match pr_url_or_id.parse::<u32>() {
      Ok(number) => number,
      Err(e) => {
        print_error(&format!("Invalid PR ID '{pr_url_or_id}': {e}"));
        return Ok(());
      }
    }
  };

  let pr = match rt.block_on(github_client.get_pull_request(&owner, &repo_name, pr_number)) {
    Ok(pr) => pr,
    Err(e) => {
      print_error(&format!("Failed to get PR: {e}"));
      return Ok(());
    }
  };

  // Get the current branch
  let head = match repo.head() {
    Ok(head) => head,
    Err(e) => {
      print_error(&format!("Failed to get repository HEAD: {e}"));
      return Ok(());
    }
  };

  let branch_name = match head.shorthand() {
    Some(name) => name,
    None => {
      print_error("Failed to get branch name");
      return Ok(());
    }
  };

  // Load the repository state
  let mut repo_state = match RepoState::load(&repo_path) {
    Ok(state) => state,
    Err(e) => {
      print_error(&format!("Failed to load repository state: {e}"));
      return Ok(());
    }
  };

  // Check if the branch already has an associated issue
  let now = chrono::Utc::now().to_rfc3339();

  if let Some(branch_issue) = repo_state.get_branch_issue_by_branch(branch_name) {
    // Update the existing branch issue
    let mut updated_branch_issue = branch_issue.clone();
    updated_branch_issue.github_pr = Some(pr_number);

    repo_state.add_branch_issue(updated_branch_issue);
    print_success(&format!(
      "Updated branch '{branch_name}' to link with PR #{pr_number}: {}",
      pr.title
    ));
  } else {
    // Create a new branch issue
    let branch_issue = crate::repo_state::BranchMetadata {
      branch: branch_name.to_string(),
      jira_issue: None,
      github_pr: Some(pr_number),
      created_at: now,
    };

    repo_state.add_branch_issue(branch_issue);
    print_success(&format!(
      "Linked branch '{branch_name}' with PR #{pr_number}: {}",
      pr.title
    ));
  }

  // Save the repository state
  match repo_state.save(&repo_path) {
    Ok(_) => Ok(()),
    Err(e) => {
      print_error(&format!("Failed to save repository state: {e}"));
      Ok(())
    }
  }
}

/// Display PR status information
fn display_pr_status(status: &PullRequestStatus) {
  let pr = &status.pr;

  println!();
  print_info(&format!("Pull Request #{}", pr.number));
  println!("  Title: {}", pr.title);
  println!("  URL: {}", pr.html_url);
  println!("  State: {}", pr.state);

  if let Some(draft) = pr.draft {
    if draft {
      println!("  Draft: Yes");
    }
  }

  println!("  Created: {}", pr.created_at);
  println!("  Updated: {}", pr.updated_at);

  if let Some(mergeable) = pr.mergeable {
    println!("  Mergeable: {}", if mergeable { "Yes" } else { "No" });
  }

  if let Some(mergeable_state) = &pr.mergeable_state {
    println!("  Mergeable State: {mergeable_state}");
  }

  // Display reviews
  if !status.reviews.is_empty() {
    println!();
    print_info("Reviews:");

    // Group reviews by user and get the latest review for each user
    let mut latest_reviews = std::collections::HashMap::new();

    for review in &status.reviews {
      let entry = latest_reviews
        .entry(review.user.login.clone())
        .or_insert_with(|| review);

      // Update if this review is newer
      if review.submitted_at > entry.submitted_at {
        *entry = review;
      }
    }

    for (_, review) in latest_reviews {
      let formatted_state = format_pr_review_status(&review.state);

      println!(
        "  {} by {}: {}",
        review.submitted_at, review.user.login, formatted_state,
      );
    }
  }

  // Display check runs
  if !status.check_runs.is_empty() {
    println!();
    print_info("Checks:");

    for check in &status.check_runs {
      let status_str = format_check_status(&check.status, check.conclusion.as_deref());

      println!("  {}: {}", check.name, status_str);
    }
  }

  println!();
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_github_command_factory() {
    let cmd = GitHubCommand::command();
    assert_eq!(cmd.get_name(), "github");
    let about = cmd.get_about().unwrap().to_string();
    assert!(about.contains("GitHub integration"));
  }
}
