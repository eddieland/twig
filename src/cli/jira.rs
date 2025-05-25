use anyhow::{Context, Result};
use clap::{Arg, Command};
use colored::Colorize;
use tokio::runtime::Runtime;

use crate::api::jira::create_jira_client;
use crate::creds::get_jira_credentials;
use crate::utils::output::{format_command, print_error, print_info, print_warning};

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
        ),
    )
}

/// Handle jira subcommands
pub fn handle_commands(jira_matches: &clap::ArgMatches) -> Result<()> {
  match jira_matches.subcommand() {
    Some(("issue", issue_matches)) => handle_issue_commands(issue_matches),
    _ => {
      print_warning("Unknown jira command.");
      println!("Use {} for usage information.", format_command("--help"));
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
    _ => {
      print_warning("Unknown issue command.");
      println!("Use {} for usage information.", format_command("--help"));
      Ok(())
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
