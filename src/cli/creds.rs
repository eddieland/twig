use std::fs::metadata;
use std::os::unix::fs::PermissionsExt;

use anyhow::Result;
use clap::Command;

use crate::creds::{check_github_credentials, check_jira_credentials, get_netrc_path};
use crate::utils::output::{format_command, format_repo_path, print_error, print_info, print_success, print_warning};

/// Build the credentials subcommand
pub fn build_command() -> Command {
  Command::new("creds")
    .about("Credential management")
    .long_about(
      "Manage credentials for external services like Jira and GitHub.\n\n\
            This command group helps you check and set up credentials for the\n\
            external services that twig integrates with. Credentials are stored\n\
            in your .netrc file for security and compatibility with other tools.",
    )
    .arg_required_else_help(true)
    .subcommand(
      Command::new("check")
        .about("Check if credentials are properly configured")
        .long_about(
          "Checks if credentials for Jira and GitHub are properly configured.\n\n\
                    This command verifies that your .netrc file contains the necessary\n\
                    credentials for the services that twig integrates with. It also checks\n\
                    file permissions to ensure your credentials are secure.",
        ),
    )
    .subcommand(
      Command::new("setup")
        .about("Set up credentials interactively")
        .long_about(
          "Interactive wizard to set up credentials for Jira and GitHub.\n\n\
                    This command guides you through the process of setting up credentials\n\
                    for the services that twig integrates with. It will create or update\n\
                    your .netrc file with the provided credentials.",
        )
        .hide(true), // Hide this command until it's implemented
    )
}

/// Handle credentials subcommands
pub fn handle_commands(creds_matches: &clap::ArgMatches) -> Result<()> {
  match creds_matches.subcommand() {
    Some(("check", _)) => handle_check_command(),
    Some(("setup", _)) => handle_setup_command(),
    _ => {
      print_warning("Unknown credentials command.");
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
  let netrc_path = get_netrc_path();

  // Check if .netrc file exists
  if !netrc_path.exists() {
    print_error("No .netrc file found.");
    println!(
      "Create a .netrc file at {} with your credentials.",
      format_repo_path(&netrc_path.display().to_string())
    );
    return Ok(());
  }

  // Check file permissions
  let metadata = metadata(&netrc_path)?;
  let permissions = metadata.permissions();
  let mode = permissions.mode();

  if mode & 0o077 != 0 {
    print_warning("Your .netrc file has insecure permissions.");
    println!(
      "For security, change permissions to 600: {}",
      format_command(&format!("chmod 600 {}", netrc_path.display()))
    );
  } else {
    print_success(".netrc file has secure permissions.");
  }

  // Check Jira credentials
  match check_jira_credentials() {
    Ok(true) => print_success("Jira credentials found."),
    Ok(false) => {
      print_warning("No Jira credentials found.");
      println!("Add credentials for machine 'atlassian.com' to your .netrc file.");
    }
    Err(e) => print_error(&format!("Error checking Jira credentials: {e}")),
  }

  // Check GitHub credentials
  match check_github_credentials() {
    Ok(true) => print_success("GitHub credentials found."),
    Ok(false) => {
      print_warning("No GitHub credentials found.");
      println!("Add credentials for machine 'github.com' to your .netrc file.");
    }
    Err(e) => print_error(&format!("Error checking GitHub credentials: {e}")),
  }

  // Print .netrc format example
  print_info("Example .netrc format:");
  println!("```");
  println!("machine atlassian.com");
  println!("  login your-email@example.com");
  println!("  password your-api-token");
  println!();
  println!("machine github.com");
  println!("  login your-github-username");
  println!("  password your-personal-access-token");
  println!("```");

  Ok(())
}

/// Handle the setup command (placeholder for future implementation)
fn handle_setup_command() -> Result<()> {
  print_info("Interactive credential setup will be implemented in a future version.");
  print_info("For now, please manually edit your .netrc file.");

  Ok(())
}
