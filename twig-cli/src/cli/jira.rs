//! # Jira Command
//!
//! Derive-based implementation of the Jira command for Jira integration,
//! including issue viewing, transitioning, and branch creation.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use directories::BaseDirs;
use git2::Repository as Git2Repository;
use owo_colors::OwoColorize;
use twig_core::jira_parser::JiraTicketParser;
use twig_core::output::{print_error, print_info, print_success, print_warning};
use twig_core::{
  BranchMetadata, RepoState, create_jira_parser, create_worktree, detect_repository, get_config_dirs,
  get_current_branch_jira_issue,
};
use twig_jira::{create_jira_runtime_and_client, get_jira_host};

/// Command for Jira integration
#[derive(Args)]
pub struct JiraArgs {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: JiraSubcommands,
}

/// Subcommands for the Jira command
#[derive(Subcommand)]
pub enum JiraSubcommands {
  /// Open Jira issue in browser
  #[command(long_about = "Open a Jira issue in the default browser.\n\n\
                         If no issue key is provided, opens the issue associated with the current branch.\n\
                         The command will construct the Jira URL using the configured host and open it using the system's default browser.")]
  Open {
    /// The Jira issue key (e.g., PROJ-123, proj123, Me-1234)
    #[arg(index = 1)]
    issue_key: Option<String>,
  },

  /// Create a branch from a Jira issue
  #[command(long_about = "Create a Git branch from a Jira issue.\n\n\
                      This command creates a branch with a name derived from the Jira issue key\n\
                      and summary, and associates the branch with the issue in the repository state.")]
  CreateBranch {
    /// The Jira issue key (e.g., PROJ-123, proj123, Me-1234)
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
    #[arg(
      index = 1,
      long_help = "The Jira issue key (e.g., PROJ-123)\n\
                 If not provided, uses the current branch's associated Jira issue"
    )]
    issue_key: Option<String>,

    /// The branch name (if not provided, the current branch will be used)
    #[arg(index = 2)]
    branch_name: Option<String>,
  },

  /// Transition a Jira issue
  #[command(long_about = "Transition a Jira issue to a different status.\n\n\
                       This command allows you to move a Jira issue through its workflow.\n\
                       If no transition is specified, it will list available transitions.")]
  Transition {
    #[arg(
      index = 1,
      long_help = "The Jira issue key (e.g., PROJ-123)\n\
                 If not provided, uses the current branch's associated Jira issue"
    )]
    issue_key: Option<String>,

    #[arg(
      index = 2,
      long_help = "The transition name or ID (if not provided, available transitions will be listed)"
    )]
    transition: Option<String>,
  },

  /// View a Jira issue
  #[command(long_about = "View details of a Jira issue.\n\n\
                   This command displays information about a specific Jira issue,\n\
                   including its key, summary, status, and description.")]
  View {
    #[arg(
      index = 1,
      long_help = "The Jira issue key (e.g., PROJ-123)\n\
                 If not provided, uses the current branch's associated Jira issue"
    )]
    issue_key: Option<String>,
  },

  /// Configure Jira parsing settings
  #[command(long_about = "Configure Jira ticket parsing behavior.\n\n\
                         Allows you to set parsing mode (strict/flexible) and other preferences.")]
  Config {
    /// Set the parsing mode
    #[arg(long, value_enum)]
    mode: Option<JiraParsingModeArg>,

    /// Show current configuration
    #[arg(long)]
    show: bool,
  },
}

/// Jira parsing mode argument for CLI
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum JiraParsingModeArg {
  /// Strict mode: Only accepts ME-1234 format
  Strict,
  /// Flexible mode: Accepts ME-1234, ME1234, me1234, etc.
  Flexible,
}

impl From<JiraParsingModeArg> for twig_core::jira_parser::JiraParsingMode {
  fn from(mode: JiraParsingModeArg) -> Self {
    match mode {
      JiraParsingModeArg::Strict => twig_core::jira_parser::JiraParsingMode::Strict,
      JiraParsingModeArg::Flexible => twig_core::jira_parser::JiraParsingMode::Flexible,
    }
  }
}

/// Handle the Jira command
///
/// This function processes the Jira subcommands and executes the appropriate
/// actions based on the subcommand provided.
pub(crate) fn handle_jira_command(jira: JiraArgs) -> Result<()> {
  // Create Jira parser once for the entire command
  let jira_parser = create_jira_parser();

  match jira.subcommand {
    JiraSubcommands::Open { issue_key } => handle_jira_open_command(jira_parser.as_ref(), issue_key.as_deref()),
    JiraSubcommands::CreateBranch {
      issue_key,
      with_worktree,
    } => {
      // Parse and normalize the issue key
      match jira_parser
        .as_ref()
        .and_then(|parser| parse_and_validate_issue_key(parser, &issue_key))
      {
        Some(normalized_key) => handle_create_branch_command(&normalized_key, with_worktree),
        None => {
          print_error(&format!("Invalid Jira issue key format: '{issue_key}'"));
          Ok(())
        }
      }
    }
    JiraSubcommands::LinkBranch { issue_key, branch_name } => {
      match issue_key {
        Some(key) => {
          // Parse and normalize the issue key
          match jira_parser
            .as_ref()
            .and_then(|parser| parse_and_validate_issue_key(parser, &key))
          {
            Some(normalized_key) => handle_link_branch_command(&normalized_key, branch_name.as_deref()),
            None => {
              print_error(&format!("Invalid Jira issue key format: '{key}'"));
              Ok(())
            }
          }
        }
        None => {
          // Try to get the Jira issue from the current branch
          match get_current_branch_jira_issue() {
            Ok(Some(key)) => handle_link_branch_command(&key, branch_name.as_deref()),
            Ok(None) => {
              print_error("No Jira issue key provided and current branch has no associated Jira issue");
              Ok(())
            }
            Err(e) => {
              print_error(&format!("Failed to get associated Jira issue: {e}"));
              Ok(())
            }
          }
        }
      }
    }
    JiraSubcommands::Transition { issue_key, transition } => {
      match issue_key {
        Some(key) => {
          // Parse and normalize the issue key
          match jira_parser
            .as_ref()
            .and_then(|parser| parse_and_validate_issue_key(parser, &key))
          {
            Some(normalized_key) => handle_transition_issue_command(&normalized_key, transition.as_deref()),
            None => {
              print_error(&format!("Invalid Jira issue key format: '{key}'"));
              Ok(())
            }
          }
        }
        None => {
          // Try to get the Jira issue from the current branch
          match get_current_branch_jira_issue() {
            Ok(Some(key)) => handle_transition_issue_command(&key, transition.as_deref()),
            Ok(None) => {
              print_error("No Jira issue key provided and current branch has no associated Jira issue");
              Ok(())
            }
            Err(e) => {
              print_error(&format!("Failed to get associated Jira issue: {e}"));
              Ok(())
            }
          }
        }
      }
    }
    JiraSubcommands::View { issue_key } => {
      // If issue_key is None, try to get it from the current branch
      match issue_key {
        Some(key) => {
          // Parse and normalize the issue key
          match jira_parser
            .as_ref()
            .and_then(|parser| parse_and_validate_issue_key(parser, &key))
          {
            Some(normalized_key) => handle_view_issue_command(&normalized_key),
            None => {
              print_error(&format!("Invalid Jira issue key format: '{key}'"));
              Ok(())
            }
          }
        }
        None => {
          // Try to get the Jira issue from the current branch
          match get_current_branch_jira_issue() {
            Ok(Some(key)) => handle_view_issue_command(&key),
            Ok(None) => {
              print_error("No Jira issue key provided and current branch has no associated Jira issue");
              Ok(())
            }
            Err(e) => {
              print_error(&format!("Failed to get associated Jira issue: {e}"));
              Ok(())
            }
          }
        }
      }
    }
    JiraSubcommands::Config { mode, show } => {
      if show {
        handle_show_jira_config()
      } else if let Some(mode) = mode {
        handle_set_jira_config(mode)
      } else {
        print_error("Please specify --mode or --show");
        Ok(())
      }
    }
  }
}

/// Handle showing current Jira configuration
fn handle_show_jira_config() -> Result<()> {
  let config_dirs = get_config_dirs()?;
  let jira_config = config_dirs.load_jira_config()?;

  print_info("Current Jira configuration:");
  println!("  Jira Ticket Parsing: {:?}", jira_config.mode);

  Ok(())
}

/// Handle setting Jira configuration
fn handle_set_jira_config(mode: JiraParsingModeArg) -> Result<()> {
  let config_dirs = get_config_dirs()?;
  let mut jira_config = config_dirs.load_jira_config().unwrap_or_default();

  jira_config.mode = mode.into();

  config_dirs.save_jira_config(&jira_config)?;

  print_success(&format!("Jira parsing mode set to: {:?}", jira_config.mode));

  Ok(())
}

/// Handle the view issue command
fn handle_view_issue_command(issue_key: &str) -> Result<()> {
  let base_dirs = BaseDirs::new().context("Failed to get $HOME directory")?;
  let jira_host = get_jira_host()?;

  let (rt, jira_client) = create_jira_runtime_and_client(base_dirs.home_dir(), &jira_host)?;

  rt.block_on(async {
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
  let base_dirs = BaseDirs::new().context("Failed to get $HOME directory")?;
  let jira_host = get_jira_host()?;

  let (rt, jira_client) = create_jira_runtime_and_client(base_dirs.home_dir(), &jira_host)?;

  rt.block_on(async {
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
  let base_dirs = BaseDirs::new().context("Failed to get $HOME directory")?;
  let jira_host = get_jira_host()?;

  let (rt, jira_client) = create_jira_runtime_and_client(base_dirs.home_dir(), &jira_host)?;

  rt.block_on(async {
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
    let repo_path = match twig_core::detect_repository() {
      Some(path) => path,
      None => {
        print_error("Failed to find git repository");
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
      match create_worktree(&repo_path, &branch_name) {
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
  let base_dirs = BaseDirs::new().context("Failed to get $HOME directory")?;
  let jira_host = get_jira_host()?;

  let (rt, jira_client) = create_jira_runtime_and_client(base_dirs.home_dir(), &jira_host)?;
  rt.block_on(async {
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
    let repo_path = match detect_repository() {
      Some(path) => path,
      None => {
        print_error("Failed to find git repository");
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
    if let Some(existing) = state.get_branch_metadata(&branch) {
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

/// Parse and validate a Jira issue key using the provided parser
fn parse_and_validate_issue_key(parser: &JiraTicketParser, input: &str) -> Option<String> {
  parser.parse(input).ok()
}

/// Handle the Jira open command
fn handle_jira_open_command(jira_parser: Option<&JiraTicketParser>, issue_key: Option<&str>) -> Result<()> {
  use twig_core::open_url_in_browser;

  // Determine issue key (from arg or current branch)
  let issue_key = if let Some(key) = issue_key {
    // Parse and normalize the provided key
    match jira_parser.and_then(|parser| parse_and_validate_issue_key(parser, key)) {
      Some(normalized_key) => normalized_key,
      None => {
        print_error(&format!("Invalid Jira issue key format: '{key}'"));
        return Ok(());
      }
    }
  } else {
    // Try to get the Jira issue from the current branch
    match get_current_branch_jira_issue() {
      Ok(Some(key)) => key,
      Ok(None) => {
        print_error("Current branch has no associated Jira issue");
        print_info("Link an issue with: twig jira link-branch <issue-key>");
        return Ok(());
      }
      Err(e) => {
        print_error(&format!("Failed to get associated Jira issue: {e}"));
        return Ok(());
      }
    }
  };

  // Get Jira host from configuration
  let jira_host = match get_jira_host() {
    Ok(host) => host,
    Err(e) => {
      print_error(&format!("Jira host not configured: {e}"));
      print_info("Set up Jira credentials with: twig creds jira");
      return Ok(());
    }
  };

  // Construct Jira issue URL
  let url = format!("{jira_host}/browse/{issue_key}");

  // Open URL in browser
  open_url_in_browser(&url)
}
