use anyhow::Result;
use clap::Command;
use tokio::runtime::Runtime;

use crate::api::github::create_github_client;
use crate::creds::get_github_credentials;
use crate::utils::output::{print_error, print_info, print_success};

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
}

/// Handle GitHub commands
pub fn handle_commands(github_matches: &clap::ArgMatches) -> Result<()> {
  match github_matches.subcommand() {
    Some(("check", _)) => handle_check_command(),
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
