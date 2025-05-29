//! # Jira Command
//!
//! CLI commands for Jira integration, including issue management, transitions,
//! and synchronization with branch metadata for workflow automation.

use anyhow::{Context, Result};
use clap::{Arg, Command};
use colored::Colorize;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tokio::runtime::Runtime;
use twig_jira::create_jira_client;

use crate::creds::get_jira_credentials;
use crate::utils::output::{print_error, print_info, print_success, print_warning};

/// Build the jira subcommand
pub fn build_command() -> Command {
  Command::new("jira")
    .about("Jira integration")
    .long_about(
      "Integrate with Jira for issue tracking and workflow management.\n\n\
            This command group allows you to view Jira issues, create branches based on\n\
            issues, and transition issues through your workflow. It requires proper\n\
            Jira credentials to be configured in your .netrc file.",
    )
    .arg_required_else_help(true)
    .alias("j")
    .subcommand(
      Command::new("issue")
        .about("Jira issue commands")
        .long_about(
          "Commands for working with Jira issues.\n\n\
                     These commands allow you to view and manage Jira issues directly\n\
                     from the command line. You can view issue details and transition\n\
                     issues through your workflow.",
        )
        .alias("i")
        .subcommand(
          Command::new("list")
            .about("List Jira issues")
            .long_about(
              "List Jira issues with filtering options.\n\n\
                           This command displays a table of Jira issues with key information\n\
                           such as issue key, summary, status, and assignee.",
            )
            .alias("ls")
            .arg(
              Arg::new("project")
                .help("Filter by project key")
                .long("project")
                .short('p')
                .value_name("PROJECT"),
            )
            .arg(
              Arg::new("status")
                .help("Filter by issue status")
                .long("status")
                .short('s')
                .value_name("STATUS"),
            )
            .arg(
              Arg::new("assignee")
                .help("Filter by assignee (me or username)")
                .long("assignee")
                .short('a')
                .value_name("ASSIGNEE"),
            )
            .arg(
              Arg::new("limit")
                .help("Maximum number of issues to display")
                .long("limit")
                .short('l')
                .value_name("COUNT")
                .value_parser(clap::value_parser!(u32))
                .default_value("50"),
            ),
        )
        .subcommand(
          Command::new("view")
            .about("View a Jira issue")
            .long_about(
              "View details of a specific Jira issue.\n\n\
                            This command fetches and displays information about a Jira issue,\n\
                            including its summary, description, and current status. It requires\n\
                            proper Jira credentials to be configured.",
            )
            .alias("show")
            .arg(
              Arg::new("issue_key")
                .help("The Jira issue key (e.g., PROJ-123)")
                .required(true)
                .index(1),
            ),
        )
        .subcommand(
          Command::new("transition")
            .about("Transition a Jira issue to a new status")
            .long_about(
              "Transition a Jira issue to a new status in the workflow.\n\n\
                             This command allows you to move an issue through your workflow\n\
                             by transitioning it to a new status. You can specify the transition\n\
                             by name or ID. If no transition is specified, available transitions\n\
                             will be displayed.",
            )
            .alias("trans")
            .arg(
              Arg::new("issue_key")
                .help("The Jira issue key (e.g., PROJ-123)")
                .required(true)
                .index(1),
            )
            .arg(
              Arg::new("transition")
                .help("The transition name or ID (e.g., 'In Progress')")
                .index(2),
            ),
        )
        .subcommand(
          Command::new("comment")
            .about("Add a comment to a Jira issue")
            .long_about(
              "Add a comment to a Jira issue.\n\n\
                             This command allows you to add a comment to a Jira issue\n\
                             either by providing the comment text directly or by reading\n\
                             it from a file.",
            )
            .arg(
              Arg::new("issue_key")
                .help("The Jira issue key (e.g., PROJ-123)")
                .required(true)
                .index(1),
            )
            .arg(Arg::new("comment_text").help("The comment text").index(2))
            .arg(
              Arg::new("file")
                .help("Path to a file containing the comment text")
                .long("file")
                .short('f')
                .value_name("PATH")
                .conflicts_with("comment_text"),
            )
            .arg(
              Arg::new("dry_run")
                .help("Show what would be done without actually adding the comment")
                .long("dry-run")
                .action(clap::ArgAction::SetTrue),
            )
            .arg(
              Arg::new("preview")
                .help("Show comment preview before submission")
                .long("preview")
                .action(clap::ArgAction::SetTrue),
            ),
        ),
    )
    .subcommand(
      Command::new("branch")
        .about("Jira branch commands")
        .long_about(
          "Commands for working with Git branches linked to Jira issues.\n\n\
                    These commands allow you to create branches based on Jira issues\n\
                    and link existing branches to issues. This helps maintain the\n\
                    connection between code and issues.",
        )
        .alias("br")
        .subcommand(
          Command::new("create")
            .about("Create a branch for a Jira issue")
            .long_about(
              "Create a Git branch for a specific Jira issue.\n\n\
                            This command creates a new branch with a name based on the Jira\n\
                            issue key and summary. It also records the association between\n\
                            the branch and the issue in the repository state.",
            )
            .alias("new")
            .arg(
              Arg::new("issue_key")
                .help("The Jira issue key (e.g., PROJ-123)")
                .required(true)
                .index(1),
            )
            .arg(
              Arg::new("worktree")
                .help("Create a worktree for the branch")
                .long_help(
                  "Create a worktree for the branch in addition to creating the branch.\n\
                                This allows you to work on the issue in a separate directory.",
                )
                .long("worktree")
                .short('w')
                .action(clap::ArgAction::SetTrue),
            ),
        )
        .subcommand(
          Command::new("link")
            .about("Link a branch to a Jira issue")
            .long_about(
              "Link an existing Git branch to a Jira issue.\n\n\
                            This command records the association between a branch and a\n\
                            Jira issue in the repository state. This is useful for branches\n\
                            that were created outside of twig.",
            )
            .arg(
              Arg::new("issue_key")
                .help("The Jira issue key (e.g., PROJ-123)")
                .required(true)
                .index(1),
            )
            .arg(
              Arg::new("branch")
                .help("The branch name (defaults to current branch)")
                .index(2),
            ),
        ),
    )
}

/// Handle jira subcommands
pub fn handle_commands(jira_matches: &clap::ArgMatches) -> Result<()> {
  match jira_matches.subcommand() {
    Some(("issue", issue_matches)) => handle_issue_commands(issue_matches),
    Some(("branch", branch_matches)) => handle_branch_commands(branch_matches),
    _ => {
      print_warning("Unknown jira command.");
      Ok(())
    }
  }
}

/// Handle branch subcommands
fn handle_branch_commands(branch_matches: &clap::ArgMatches) -> Result<()> {
  match branch_matches.subcommand() {
    Some(("create", create_matches)) => {
      let issue_key = create_matches.get_one::<String>("issue_key").unwrap();
      let create_worktree = create_matches.get_flag("worktree");
      handle_create_branch_command(issue_key, create_worktree)
    }
    Some(("link", link_matches)) => {
      let issue_key = link_matches.get_one::<String>("issue_key").unwrap();
      let branch = link_matches.get_one::<String>("branch");
      handle_link_branch_command(issue_key, branch.map(|s| s.as_str()))
    }
    _ => {
      print_warning("Unknown branch command.");
      Ok(())
    }
  }
}

/// Handle issue subcommands
fn handle_issue_commands(issue_matches: &clap::ArgMatches) -> Result<()> {
  match issue_matches.subcommand() {
    Some(("view", view_matches)) => {
      let issue_key = view_matches.get_one::<String>("issue_key").unwrap();
      handle_view_issue_command(issue_key)
    }
    Some(("list", list_matches)) => handle_list_issues_command(list_matches),
    Some(("transition", transition_matches)) => {
      let issue_key = transition_matches.get_one::<String>("issue_key").unwrap();
      let transition = transition_matches.get_one::<String>("transition");
      handle_transition_issue_command(issue_key, transition.map(|s| s.as_str()))
    }
    Some(("comment", comment_matches)) => handle_comment_issue_command(comment_matches),
    _ => {
      print_warning("Unknown issue command.");
      Ok(())
    }
  }
}

/// Handle the list issues command
fn handle_list_issues_command(list_matches: &clap::ArgMatches) -> Result<()> {
  use colored::Colorize;

  // Create a tokio runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  rt.block_on(async {
    // Get Jira credentials
    let creds = match get_jira_credentials() {
      Ok(creds) => creds,
      Err(e) => {
        print_error(&format!("Failed to get Jira credentials: {e}"));
        print_info("Use the 'twig creds check' command to verify your credentials.");
        return Ok(());
      }
    };

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var("JIRA_HOST").unwrap_or_else(|_| "https://eddieland.atlassian.net".to_string());

    // Create Jira client
    let jira_client = create_jira_client(&jira_host, &creds.username, &creds.password)?;

    // Get filter parameters
    let project = list_matches.get_one::<String>("project").map(|s| s.as_str());
    let status = list_matches.get_one::<String>("status").map(|s| s.as_str());
    let assignee = list_matches.get_one::<String>("assignee").map(|s| s.as_str());
    let limit = list_matches.get_one::<u32>("limit").copied().unwrap_or(50);

    // Set up pagination
    let pagination = Some((limit, 0));

    // Build filter description for output
    let mut filters = Vec::new();
    if let Some(p) = project {
      filters.push(format!("project={p}"));
    }
    if let Some(s) = status {
      filters.push(format!("status=\"{s}\""));
    }
    if let Some(a) = assignee {
      if a == "me" {
        filters.push("assignee=me".to_string());
      } else {
        filters.push(format!("assignee=\"{a}\""));
      }
    }

    let filter_desc = if filters.is_empty() {
      "all".to_string()
    } else {
      filters.join(", ")
    };

    // Fetch issues
    println!("Fetching Jira issues with filters: {filter_desc}");

    match jira_client.list_issues(project, status, assignee, pagination).await {
      Ok(issues) => {
        if issues.is_empty() {
          println!("No issues found matching the specified filters");
          return Ok(());
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

        // Convert issues to table rows
        let rows: Vec<IssueRow> = issues
          .into_iter()
          .map(|issue| {
            // Truncate summary if too long
            let summary = if issue.fields.summary.len() > 47 {
              format!("{}...", &issue.fields.summary[0..44])
            } else {
              issue.fields.summary.clone()
            };

            // Format status with color
            let status_colored = match issue.fields.status.name.as_str() {
              "To Do" | "Open" | "New" => issue.fields.status.name.blue().to_string(),
              "In Progress" | "In Review" => issue.fields.status.name.yellow().to_string(),
              "Done" | "Closed" | "Resolved" => issue.fields.status.name.green().to_string(),
              _ => issue.fields.status.name.normal().to_string(),
            };

            // Get assignee
            let assignee = issue
              .fields
              .assignee
              .map(|a| a.display_name)
              .unwrap_or_else(|| "Unassigned".to_string());

            IssueRow {
              key: issue.key.bold().to_string(),
              summary,
              status: status_colored,
              assignee,
            }
          })
          .collect();

        // Create and display the table with a simpler style
        println!();
        println!("{}", Table::new(rows).with(Style::ascii()));
        println!();
      }
      Err(e) => {
        print_error(&format!("Failed to fetch issues: {e}"));
      }
    }

    Ok(())
  })
}

/// Handle the comment issue command
fn handle_comment_issue_command(comment_matches: &clap::ArgMatches) -> Result<()> {
  use std::fs;
  use std::io::{self, Read};

  // Create a tokio runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  rt.block_on(async {
    // Get issue key
    let issue_key = comment_matches.get_one::<String>("issue_key").unwrap();

    // Get comment text from argument, file, or stdin
    let comment_text = if let Some(text) = comment_matches.get_one::<String>("comment_text") {
      text.clone()
    } else if let Some(file_path) = comment_matches.get_one::<String>("file") {
      match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
          print_error(&format!("Failed to read comment file: {e}"));
          return Ok(());
        }
      }
    } else {
      // If no comment text or file is provided, read from stdin
      println!("Enter comment text (press Ctrl+D when finished):");
      let mut buffer = String::new();
      match io::stdin().read_to_string(&mut buffer) {
        Ok(_) => buffer,
        Err(e) => {
          print_error(&format!("Failed to read from stdin: {e}"));
          return Ok(());
        }
      }
    };

    // Check if comment is empty
    if comment_text.trim().is_empty() {
      print_error("Comment text cannot be empty");
      return Ok(());
    }

    // Preview comment if requested
    let preview = comment_matches.get_flag("preview");
    if preview {
      println!("\nComment Preview for {}: ", issue_key.bold());
      println!("{comment_text}\n");
      println!("Press Enter to continue or Ctrl+C to cancel...");
      let mut input = String::new();
      io::stdin().read_line(&mut input).ok();
    }

    // Get dry run flag
    let dry_run = comment_matches.get_flag("dry_run");

    // Get Jira credentials
    let creds = match get_jira_credentials() {
      Ok(creds) => creds,
      Err(e) => {
        print_error(&format!("Failed to get Jira credentials: {e}"));
        print_info("Use the 'twig creds check' command to verify your credentials.");
        return Ok(());
      }
    };

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var("JIRA_HOST").unwrap_or_else(|_| "https://eddieland.atlassian.net".to_string());

    // Create Jira client
    let jira_client = create_jira_client(&jira_host, &creds.username, &creds.password)?;

    // Add comment
    if dry_run {
      println!("Dry run: Would add comment to issue {issue_key}");
      println!("Comment text: {comment_text}");
      return Ok(());
    }

    match jira_client.add_comment(issue_key, &comment_text, false).await {
      Ok(Some(comment)) => {
        print_success(&format!("Successfully added comment to issue {issue_key}"));
        print_info(&format!("Comment ID: {}", comment.id));
        print_info(&format!("Created: {}", comment.created));
        Ok(())
      }
      Ok(None) => {
        // This shouldn't happen since dry_run is false
        print_warning("No comment was added (dry run)");
        Ok(())
      }
      Err(e) => {
        print_error(&format!("Failed to add comment: {e}"));
        Ok(())
      }
    }
  })
}

/// Handle the view issue command
fn handle_view_issue_command(issue_key: &str) -> Result<()> {
  // Create a tokio runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  rt.block_on(async {
    // Get Jira credentials
    let creds = match get_jira_credentials() {
      Ok(creds) => creds,
      Err(e) => {
        print_error(&format!("Failed to get Jira credentials: {e}"));
        print_info("Use the 'twig creds check' command to verify your credentials.");
        return Ok(());
      }
    };

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var("JIRA_HOST").unwrap_or_else(|_| "https://eddieland.atlassian.net".to_string());

    // Create Jira client
    let jira_client = create_jira_client(&jira_host, &creds.username, &creds.password)?;

    // Fetch the issue
    match jira_client.get_issue(issue_key).await {
      Ok(issue) => {
        // Create a cleaner, more elegant output for the Jira issue
        let title = format!(" Jira Issue: {} ", issue.key);
        let title_len = title.len();
        let line_width = 78;
        let left_padding = (line_width - title_len) / 2;
        let right_padding = line_width - title_len - left_padding;

        // Print a clear header for the issue
        println!(
          "\n{}{}{}",
          "─".repeat(left_padding),
          title.blue().bold(),
          "─".repeat(right_padding)
        );

        // Format key fields with clear labels and indentation
        println!("\n  {} {}", "•".blue(), format!("Key:     {}", issue.key).bold());
        println!(
          "  {} {}",
          "•".blue(),
          format!("Summary: {}", issue.fields.summary).bold()
        );
        println!(
          "  {} {}",
          "•".blue(),
          format!("Status:  {}", issue.fields.status.name).yellow().bold()
        );

        // Add description in its own section if available
        if let Some(description) = &issue.fields.description {
          let desc_title = " DESCRIPTION ";
          let desc_title_len = desc_title.len();
          let desc_left_padding = (line_width - desc_title_len) / 2;
          let desc_right_padding = line_width - desc_title_len - desc_left_padding;

          println!(
            "\n{}{}{}",
            "─".repeat(desc_left_padding),
            desc_title.blue().bold(),
            "─".repeat(desc_right_padding)
          );

          // Indent the description for better readability
          println!();
          for line in description.lines() {
            println!("  {line}");
          }
          println!();
        }

        println!("{}\n", "─".repeat(line_width));
        Ok(())
      }
      Err(e) => {
        print_error(&format!("Failed to fetch issue {issue_key}: {e}"));
        Ok(())
      }
    }
  })
}

/// Handle the transition issue command
fn handle_transition_issue_command(issue_key: &str, transition: Option<&str>) -> Result<()> {
  use crate::utils::output::{print_error, print_info, print_success};

  // Create a tokio runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  rt.block_on(async {
    // Get Jira credentials
    let creds = match get_jira_credentials() {
      Ok(creds) => creds,
      Err(e) => {
        print_error(&format!("Failed to get Jira credentials: {e}"));
        print_info("Use the 'twig creds check' command to verify your credentials.");
        return Ok(());
      }
    };

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var("JIRA_HOST").unwrap_or_else(|_| "https://eddieland.atlassian.net".to_string());

    // Create Jira client
    let jira_client = create_jira_client(&jira_host, &creds.username, &creds.password)?;

    // If no transition is specified, list available transitions
    if transition.is_none() {
      print_info(&format!("Available transitions for issue {issue_key}:"));

      match jira_client.get_transitions(issue_key).await {
        Ok(transitions) => {
          if transitions.is_empty() {
            print_info("No transitions available for this issue.");
          } else {
            for t in transitions {
              println!("  • {} (ID: {})", t.name, t.id);
            }
          }
        }
        Err(e) => {
          print_error(&format!("Failed to fetch transitions: {e}"));
          return Ok(());
        }
      }

      return Ok(());
    }

    // Get the transition ID from the name
    let transition_name = transition.unwrap();
    let transitions = match jira_client.get_transitions(issue_key).await {
      Ok(t) => t,
      Err(e) => {
        print_error(&format!("Failed to fetch transitions: {e}"));
        return Ok(());
      }
    };

    // Find the transition ID by name (case-insensitive)
    let transition_id = transitions
      .iter()
      .find(|t| t.name.to_lowercase() == transition_name.to_lowercase() || t.id == transition_name)
      .map(|t| t.id.clone());

    match transition_id {
      Some(id) => {
        // Perform the transition
        match jira_client.transition_issue(issue_key, &id).await {
          Ok(_) => {
            print_success(&format!(
              "Successfully transitioned issue {issue_key} to '{transition_name}'"
            ));
            Ok(())
          }
          Err(e) => {
            print_error(&format!("Failed to transition issue: {e}"));
            Ok(())
          }
        }
      }
      None => {
        print_error(&format!(
          "Transition '{transition_name}' not found for issue {issue_key}"
        ));
        print_info("Available transitions:");
        for t in transitions {
          println!("  • {} (ID: {})", t.name, t.id);
        }
        Ok(())
      }
    }
  })
}

/// Handle the create branch command
fn handle_create_branch_command(issue_key: &str, with_worktree: bool) -> Result<()> {
  use git2::Repository as Git2Repository;

  use crate::repo_state::{BranchMetadata, RepoState};
  use crate::utils::output::{print_error, print_info, print_success};

  // Create a tokio runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  rt.block_on(async {
    // Get Jira credentials
    let creds = match get_jira_credentials() {
      Ok(creds) => creds,
      Err(e) => {
        print_error(&format!("Failed to get Jira credentials: {e}"));
        print_info("Use the 'twig creds check' command to verify your credentials.");
        return Ok(());
      }
    };

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var("JIRA_HOST").unwrap_or_else(|_| "https://eddieland.atlassian.net".to_string());

    // Create Jira client
    let jira_client = create_jira_client(&jira_host, &creds.username, &creds.password)?;

    // Fetch the issue to get its summary
    let issue = match jira_client.get_issue(issue_key).await {
      Ok(issue) => issue,
      Err(e) => {
        print_error(&format!("Failed to fetch issue {issue_key}: {e}"));
        return Ok(());
      }
    };

    // Create a branch name from the issue key and summary
    let summary = issue.fields.summary.to_lowercase();

    // Sanitize the summary for use in a branch name
    let sanitized_summary = summary
      .chars()
      .map(|c| match c {
        ' ' | '-' | '_' => '-',
        c if c.is_alphanumeric() => c,
        _ => '-',
      })
      .collect::<String>()
      .replace("--", "-")
      .trim_matches('-') // This trims both leading and trailing hyphens
      .to_string();

    // Create the branch name in the format "PROJ-123/add-feature"
    let branch_name = format!("{issue_key}/{sanitized_summary}");

    // Get the current repository
    let repo_path = match crate::git::detect_current_repository() {
      Ok(path) => path,
      Err(e) => {
        print_error(&format!("Failed to find git repository: {e}"));
        return Ok(());
      }
    };

    // Print the branch name
    print_info(&format!("Creating branch: {branch_name}"));

    // Open the repository
    let repo = Git2Repository::open(&repo_path).context("Failed to open git repository")?;

    // Get the current timestamp
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_secs();
    let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
      .unwrap()
      .to_rfc3339();

    if with_worktree {
      // Create a worktree for the branch
      match crate::repo_state::create_worktree(&repo_path, &branch_name) {
        Ok(_) => {
          print_success(&format!("Created worktree for branch '{branch_name}'"));
        }
        Err(e) => {
          print_error(&format!("Failed to create worktree: {e}"));
          return Ok(());
        }
      }
    } else {
      // Get the HEAD commit to branch from
      let head = repo.head()?;
      let target = head
        .target()
        .ok_or_else(|| anyhow::anyhow!("HEAD is not a direct reference"))?;
      let commit = repo.find_commit(target)?;

      // Create the branch
      match repo.branch(&branch_name, &commit, false) {
        Ok(_) => {
          print_success(&format!("Created branch '{branch_name}'"));
        }
        Err(e) => {
          print_error(&format!("Failed to create branch: {e}"));
          return Ok(());
        }
      }
    }

    // Load the repository state
    let mut state = RepoState::load(&repo_path)?;

    // Add the branch-issue association
    state.add_branch_issue(BranchMetadata {
      branch: branch_name.clone(),
      jira_issue: Some(issue_key.to_string()),
      github_pr: None,
      created_at: time_str,
    });

    // Save the state
    state.save(&repo_path)?;

    print_success(&format!(
      "Associated branch '{branch_name}' with Jira issue {issue_key}"
    ));

    Ok(())
  })
}

/// Handle the link branch command
fn handle_link_branch_command(issue_key: &str, branch_name: Option<&str>) -> Result<()> {
  use git2::Repository as Git2Repository;

  use crate::repo_state::{BranchMetadata, RepoState};
  use crate::utils::output::{print_error, print_info, print_success, print_warning};

  // Create a tokio runtime for async operations
  let rt = Runtime::new().context("Failed to create async runtime")?;

  rt.block_on(async {
    // Get Jira credentials
    let creds = match get_jira_credentials() {
      Ok(creds) => creds,
      Err(e) => {
        print_error(&format!("Failed to get Jira credentials: {e}"));
        print_info("Use the 'twig creds check' command to verify your credentials.");
        return Ok(());
      }
    };

    // Load environment variables from .env file
    dotenv::dotenv().ok();

    // Get Jira host from environment or use default
    let jira_host = std::env::var("JIRA_HOST").unwrap_or_else(|_| "https://eddieland.atlassian.net".to_string());

    // Create Jira client
    let jira_client = create_jira_client(&jira_host, &creds.username, &creds.password)?;

    // Verify the issue exists
    match jira_client.get_issue(issue_key).await {
      Ok(_) => {
        // Issue exists, continue
      }
      Err(e) => {
        print_error(&format!("Failed to fetch issue {issue_key}: {e}"));
        return Ok(());
      }
    };

    // Get the current repository
    let repo_path = match crate::git::detect_current_repository() {
      Ok(path) => path,
      Err(e) => {
        print_error(&format!("Failed to find git repository: {e}"));
        return Ok(());
      }
    };

    // Open the repository
    let repo = Git2Repository::open(&repo_path).context("Failed to open git repository")?;

    // Determine the branch name
    let branch = if let Some(name) = branch_name {
      // Verify the branch exists
      if repo.find_branch(name, git2::BranchType::Local).is_err() {
        print_error(&format!("Branch '{name}' not found"));
        return Ok(());
      }
      name.to_string()
    } else {
      // Get the current branch
      let head = repo.head()?;
      if !head.is_branch() {
        print_error("Not currently on a branch");
        return Ok(());
      }

      head.shorthand().unwrap_or("HEAD").to_string()
    };

    // Get the current timestamp
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_secs();
    let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(now as i64, 0)
      .unwrap()
      .to_rfc3339();

    // Load the repository state
    let mut state = RepoState::load(&repo_path)?;

    // Check if the branch is already associated with an issue
    if let Some(existing) = state.get_branch_issue_by_branch(&branch) {
      if existing.jira_issue.as_deref() == Some(issue_key) {
        print_info(&format!(
          "Branch '{branch}' is already associated with issue {issue_key}"
        ));
        return Ok(());
      } else {
        print_warning(&format!(
          "Branch '{branch}' is already associated with issue {}. Updating to {issue_key}.",
          existing.jira_issue.as_ref().unwrap_or(&"None".to_string())
        ));
      }
    }

    // Add the branch-issue association
    state.add_branch_issue(BranchMetadata {
      branch: branch.clone(),
      jira_issue: Some(issue_key.to_string()),
      github_pr: None,
      created_at: time_str,
    });

    // Save the state
    state.save(&repo_path)?;

    print_success(&format!("Associated branch '{branch}' with Jira issue {issue_key}"));

    Ok(())
  })
}
