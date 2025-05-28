use anyhow::Result;
use clap::{Arg, Command};
use git2::Repository as Git2Repository;
use tokio::runtime::Runtime;
use twig_gh::{GitHubPRStatus, create_github_client};

use crate::creds::get_github_credentials;
use crate::git::detect_current_repository;
use crate::repo_state::RepoState;
use crate::utils::output::{
  format_check_status, format_command, format_pr_review_status, print_error, print_info, print_success, print_warning,
};

/// Build the GitHub command
pub fn build_command() -> Command {
  Command::new("github")
    .about("GitHub integration")
    .alias("gh")
    .long_about(
      "Interact with GitHub repositories and pull requests.\n\n\
            This command group provides functionality for working with GitHub,\n\
            including checking authentication, viewing pull request status,\n\
            and linking branches to pull requests.",
    )
    .subcommand(Command::new("check").about("Check GitHub authentication").long_about(
      "Verify that your GitHub credentials are working correctly.\n\n\
                  This command attempts to authenticate with GitHub using your\n\
                  credentials from .netrc and displays information about the\n\
                  authenticated user if successful.",
    ))
    .subcommand(
      Command::new("pr")
        .about("Pull request operations")
        .long_about(
          "Manage GitHub pull requests.\n\n\
                    This command group provides functionality for working with GitHub pull requests,\n\
                    including viewing status and linking branches to pull requests.",
        )
        .subcommand(
          Command::new("status")
            .about("Show PR status for current branch")
            .alias("st")
            .long_about(
              "Show the status of pull requests associated with the current branch.\n\n\
                          This command displays information about any pull requests that are\n\
                          associated with the current branch, including review status and check results.",
            ),
        )
        .subcommand(
          Command::new("link")
            .about("Link a PR to the current branch")
            .long_about(
              "Link a GitHub pull request to the current branch.\n\n\
                          This command associates a GitHub pull request with the current branch,\n\
                          allowing you to easily check its status later.",
            )
            .arg(
              Arg::new("pr_url_or_id")
                .help("URL or ID of the pull request to link (e.g., 'https://github.com/owner/repo/pull/123' or '123')")
                .required(true)
                .index(1),
            ),
        ),
    )
}

/// Handle GitHub commands
pub fn handle_commands(github_matches: &clap::ArgMatches) -> Result<()> {
  match github_matches.subcommand() {
    Some(("check", _)) => handle_check_command(),
    Some(("pr", pr_matches)) => match pr_matches.subcommand() {
      Some(("status", _)) => handle_pr_status_command(),
      Some(("link", link_matches)) => {
        let pr_url_or_id = link_matches.get_one::<String>("pr_url_or_id").unwrap();
        handle_pr_link_command(pr_url_or_id)
      }
      _ => {
        print_error("Unknown PR command");
        // Print the help text directly instead of telling the user to use --help
        let mut cmd = Command::new("pr");
        cmd.print_help().expect("Failed to print help text");
        println!();
        Ok(())
      }
    },
    _ => {
      print_error("Unknown GitHub command");
      // Print the help text directly instead of telling the user to use --help
      let mut cmd = build_command();
      cmd.print_help().expect("Failed to print help text");
      println!();
      Ok(())
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
  } else {
    // Create a new branch issue
    let branch_issue = crate::repo_state::BranchMetadata {
      branch: branch_name.to_string(),
      jira_issue: None, // No Jira issue associated
      github_pr: Some(pr_number),
      created_at: now,
    };
    repo_state.add_branch_issue(branch_issue);
  }

  // Save the repository state
  if let Err(e) = repo_state.save(&repo_path) {
    print_error(&format!("Failed to save repository state: {e}"));
    return Ok(());
  }

  print_success(&format!("Linked PR #{pr_number} to branch '{branch_name}'"));
  print_info(&format!("Title: {}", pr.title));
  print_info(&format!("URL: {}", pr.html_url));

  Ok(())
}

/// Display PR status information
fn display_pr_status(status: &GitHubPRStatus) {
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
