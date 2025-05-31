//! # Jira Command
//!
//! Derive-based implementation of the Jira command for Jira integration,
//! including issue viewing, transitioning, and branch creation.

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use git2::Repository as Git2Repository;
use owo_colors::OwoColorize;
use tokio::runtime::Runtime;
use twig_jira::create_jira_client;

use crate::cli::derive::DeriveCommand;
use crate::consts::{DEFAULT_JIRA_HOST, ENV_JIRA_HOST};
use crate::creds::get_jira_credentials;
use crate::git;
use crate::repo_state::{BranchMetadata, RepoState};
use crate::utils::output::{print_error, print_info, print_success, print_warning};

/// Command for Jira integration
#[derive(Parser)]
#[command(name = "jira")]
#[command(about = "Jira integration")]
#[command(long_about = "Interact with Jira issues and create branches from them.\n\n\
            This command group provides functionality for working with Jira,\n\
            including viewing issues, transitioning issues through workflows,\n\
            and creating branches from issues.")]
pub struct JiraCommand {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: JiraSubcommands,
}

/// Subcommands for the Jira command
#[derive(Subcommand)]
pub enum JiraSubcommands {
  /// View a Jira issue
  #[command(long_about = "View details of a Jira issue.\n\n\
                   This command displays information about a specific Jira issue,\n\
                   including its key, summary, status, and description.")]
  View {
    /// The Jira issue key (e.g., PROJ-123)
    #[arg(required = true, index = 1)]
    issue_key: String,
  },

  /// Transition a Jira issue
  #[command(long_about = "Transition a Jira issue to a different status.\n\n\
                      This command allows you to move a Jira issue through its workflow.\n\
                      If no transition is specified, it will list available transitions.")]
  Transition {
    /// The Jira issue key (e.g., PROJ-123)
    #[arg(required = true, index = 1)]
    issue_key: String,

    /// The transition name or ID (if not provided, available transitions will
    /// be listed)
    #[arg(index = 2)]
    transition: Option<String>,
  },

  /// Create a branch from a Jira issue
  #[command(long_about = "Create a Git branch from a Jira issue.\n\n\
                      This command creates a branch with a name derived from the Jira issue key\n\
                      and summary, and associates the branch with the issue in the repository state.")]
  CreateBranch {
    /// The Jira issue key (e.g., PROJ-123)
    #[arg(required = true, index = 1)]
    issue_key: String,

    /// Create a worktree for the branch
    #[arg(long, short = 'w')]
    with_worktree: bool,
  },

  /// Link a branch to a Jira issue
  #[command(long_about = "Link an existing Git branch to a Jira issue.\n\n\
                      This command associates a branch with a Jira issue in the repository state,\n\
                      allowing you to track which branches correspond to which issues.")]
  LinkBranch {
    /// The Jira issue key (e.g., PROJ-123)
    #[arg(required = true, index = 1)]
    issue_key: String,

    /// The branch name (if not provided, the current branch will be used)
    #[arg(index = 2)]
    branch_name: Option<String>,
  },
}

impl JiraCommand {
  /// Creates a clap Command for this command (for backward compatibility)
  pub fn command() -> clap::Command {
    <Self as CommandFactory>::command()
  }

  /// Parses command line arguments and executes the command
  pub fn parse_and_execute(matches: &clap::ArgMatches) -> Result<()> {
    match matches.subcommand() {
      Some(("view", view_matches)) => {
        let issue_key = view_matches.get_one::<String>("issue_key").unwrap();
        let cmd = Self {
          subcommand: JiraSubcommands::View {
            issue_key: issue_key.clone(),
          },
        };
        cmd.execute()
      }
      Some(("transition", transition_matches)) => {
        let issue_key = transition_matches.get_one::<String>("issue_key").unwrap();
        let transition = transition_matches.get_one::<String>("transition").cloned();
        let cmd = Self {
          subcommand: JiraSubcommands::Transition {
            issue_key: issue_key.clone(),
            transition,
          },
        };
        cmd.execute()
      }
      Some(("create-branch", create_branch_matches)) => {
        let issue_key = create_branch_matches.get_one::<String>("issue_key").unwrap();
        let with_worktree = create_branch_matches.get_flag("with_worktree");
        let cmd = Self {
          subcommand: JiraSubcommands::CreateBranch {
            issue_key: issue_key.clone(),
            with_worktree,
          },
        };
        cmd.execute()
      }
      Some(("link-branch", link_branch_matches)) => {
        let issue_key = link_branch_matches.get_one::<String>("issue_key").unwrap();
        let branch_name = link_branch_matches.get_one::<String>("branch_name").cloned();
        let cmd = Self {
          subcommand: JiraSubcommands::LinkBranch {
            issue_key: issue_key.clone(),
            branch_name,
          },
        };
        cmd.execute()
      }
      _ => {
        print_error("Unknown jira command");
        Ok(())
      }
    }
  }
}

impl DeriveCommand for JiraCommand {
  fn execute(self) -> Result<()> {
    match self.subcommand {
      JiraSubcommands::View { issue_key } => handle_view_issue_command(&issue_key),
      JiraSubcommands::Transition { issue_key, transition } => {
        handle_transition_issue_command(&issue_key, transition.as_deref())
      }
      JiraSubcommands::CreateBranch {
        issue_key,
        with_worktree,
      } => handle_create_branch_command(&issue_key, with_worktree),
      JiraSubcommands::LinkBranch { issue_key, branch_name } => {
        handle_link_branch_command(&issue_key, branch_name.as_deref())
      }
    }
  }
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
    let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

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
    let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

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
    let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

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
    let repo_path = match git::detect_current_repository() {
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
    let jira_host = std::env::var(ENV_JIRA_HOST).unwrap_or_else(|_| DEFAULT_JIRA_HOST.to_string());

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
    let repo_path = match git::detect_current_repository() {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_jira_command_factory() {
    let cmd = JiraCommand::command();
    assert_eq!(cmd.get_name(), "jira");
    let about = cmd.get_about().unwrap().to_string();
    assert!(about.contains("Jira integration"));
  }
}
