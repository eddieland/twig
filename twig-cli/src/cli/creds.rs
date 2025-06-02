//! # Credentials Command
//!
//! Derive-based implementation of the credentials command for managing
//! credentials for external services like Jira and GitHub.

use std::fs::metadata;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;

use anyhow::Result;
use clap::{Args, Subcommand};
use tokio::runtime::Runtime;
use twig_gh::create_github_client;
use twig_jira::create_jira_client;

use crate::creds::{check_github_credentials, check_jira_credentials, get_netrc_path, write_netrc_entry};
use crate::utils::output::{format_command, format_repo_path, print_error, print_info, print_success, print_warning};

/// Command for credential management
#[derive(Args)]
pub struct CredsArgs {
  /// The subcommand to execute
  #[command(subcommand)]
  pub subcommand: CredsSubcommands,
}

/// Subcommands for the creds command
#[derive(Subcommand)]
pub enum CredsSubcommands {
  /// Check if credentials are properly configured
  #[command(
    long_about = "Checks if credentials for Jira and GitHub are properly configured.\n\n\
                      This command verifies that your .netrc file contains the necessary\n\
                      credentials for the services that twig integrates with. It also checks\n\
                      file permissions to ensure your credentials are secure."
  )]
  Check,

  /// Set up credentials interactively
  #[command(long_about = "Interactive wizard to set up credentials for Jira and GitHub.\n\n\
                      This command guides you through the process of setting up credentials\n\
                      for the services that twig integrates with. It will create or update\n\
                      your .netrc file with the provided credentials.")]
  Setup,
}

/// Handle the creds command
///
/// This function dispatches to the appropriate subcommand handler based on
/// the user's choice. It currently supports checking credentials and setting
/// them up interactively.
pub(crate) fn handle_creds_command(creds: CredsArgs) -> Result<()> {
  match creds.subcommand {
    CredsSubcommands::Check => handle_check_command(),
    CredsSubcommands::Setup => handle_setup_command(),
  }
}

/// Handle the check command
///
/// This function checks if the .netrc file exists, verifies its permissions,
/// and checks for Jira and GitHub credentials. It also prints an example
/// .netrc format for user reference.
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
      println!("Add credentials for machine 'atlassian.net' to your .netrc file.");
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
  println!("machine atlassian.net");
  println!("  login your-email@example.com");
  println!("  password your-api-token");
  println!();
  println!("machine github.com");
  println!("  login your-github-username");
  println!("  password your-personal-access-token");
  println!("```");

  Ok(())
}

/// Handle the setup command
fn handle_setup_command() -> Result<()> {
  print_info("Welcome to the twig credential setup wizard!");
  println!("This wizard will help you configure credentials for Jira and GitHub.");
  println!();

  println!("• Credentials will be stored in ~/.netrc");
  println!("• File permissions will be automatically set to 600 for security");
  println!();

  let rt = Runtime::new()?;
  let netrc_path = get_netrc_path();

  // Check if .netrc exists and warn about overwriting
  if netrc_path.exists() {
    print_warning("A .netrc file already exists.");
    print!("Do you want to add/update credentials? (y/n): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().to_lowercase().starts_with('y') {
      print_info("Setup cancelled.");
      return Ok(());
    }
  }

  println!();
  print_info("Setting up Jira credentials:");
  println!("You'll need your Atlassian domain and API token.");
  println!("To create an API token, visit: https://id.atlassian.com/manage-profile/security/api-tokens");
  println!();

  // Get Jira credentials
  print!("Enter your Jira/Atlassian email: ");
  io::stdout().flush()?;
  let mut jira_email = String::new();
  io::stdin().read_line(&mut jira_email)?;
  let jira_email = jira_email.trim().to_string();

  if jira_email.is_empty() {
    print_warning("Email cannot be empty. Skipping Jira setup.");
    println!("You can run 'twig creds setup' again to configure Jira later.");
    println!();
  } else {
    print!("Enter your Jira API token: ");
    io::stdout().flush()?;
    let mut jira_token = String::new();
    io::stdin().read_line(&mut jira_token)?;
    let jira_token = jira_token.trim().to_string();

    if jira_token.is_empty() {
      print_warning("API token cannot be empty. Skipping Jira setup.");
      println!("You can run 'twig creds setup' again to configure Jira later.");
      println!();
    } else {
      print!("Enter your Jira domain (e.g., mycompany.atlassian.net): ");
      io::stdout().flush()?;
      let mut jira_domain = String::new();
      io::stdin().read_line(&mut jira_domain)?;
      let jira_domain = jira_domain.trim().to_string();

      if jira_domain.is_empty() {
        print_warning("Domain cannot be empty. Skipping Jira setup.");
        println!("You can run 'twig creds setup' again to configure Jira later.");
        println!();
      } else {
        // Validate Jira credentials
        print_info("Validating Jira credentials...");
        let jira_url = if jira_domain.starts_with("http") {
          jira_domain.clone()
        } else {
          format!("https://{jira_domain}")
        };

        let client = create_jira_client(&jira_url, &jira_email, &jira_token);
        match rt.block_on(client.test_connection()) {
          Ok(true) => {
            print_success("Jira credentials validated successfully!");
            write_netrc_entry("atlassian.net", &jira_email, &jira_token)?;
          }
          Ok(false) => {
            print_error("Failed to validate Jira credentials. Please check your credentials and domain.");
            print_info("Common issues:");
            println!("  • Make sure your email is correct");
            println!("  • Verify your API token is valid and not expired");
            println!("  • Check that the domain is correct (e.g., mycompany.atlassian.net)");
            print_info("You can manually add credentials to your .netrc file later.");
          }
          Err(e) => {
            print_error(&format!("Error validating Jira credentials: {e}"));
            print_info("This might be a network issue or the Jira instance might be unreachable.");
            print_info("You can manually add credentials to your .netrc file later.");
          }
        }
      }
    }
  }

  println!();
  print_info("Setting up GitHub credentials:");
  println!("You'll need your GitHub username and a Personal Access Token.");
  println!("To create a PAT, visit: https://github.com/settings/tokens");
  println!("Required scopes: repo, read:user");
  println!();

  // Get GitHub credentials
  print!("Enter your GitHub username: ");
  io::stdout().flush()?;
  let mut github_username = String::new();
  io::stdin().read_line(&mut github_username)?;
  let github_username = github_username.trim().to_string();

  if github_username.is_empty() {
    print_warning("Username cannot be empty. Skipping GitHub setup.");
    println!("You can run 'twig creds setup' again to configure GitHub later.");
    println!();
  } else {
    print!("Enter your GitHub Personal Access Token: ");
    io::stdout().flush()?;
    let mut github_token = String::new();
    io::stdin().read_line(&mut github_token)?;
    let github_token = github_token.trim().to_string();

    if github_token.is_empty() {
      print_warning("Personal Access Token cannot be empty. Skipping GitHub setup.");
      println!("You can run 'twig creds setup' again to configure GitHub later.");
      println!();
    } else {
      // Validate GitHub credentials
      print_info("Validating GitHub credentials...");
      let client = create_github_client(&github_username, &github_token);
      match rt.block_on(client.test_connection()) {
        Ok(true) => {
          print_success("GitHub credentials validated successfully!");
          write_netrc_entry("github.com", &github_username, &github_token)?;
        }
        Ok(false) => {
          print_error("Failed to validate GitHub credentials. Please check your username and token.");
          print_info("Common issues:");
          println!("  • Make sure your username is correct");
          println!("  • Verify your Personal Access Token is valid and not expired");
          println!("  • Check that the token has required scopes: repo, read:user");
          print_info("You can manually add credentials to your .netrc file later.");
        }
        Err(e) => {
          print_error(&format!("Error validating GitHub credentials: {e}",));
          print_info("This might be a network issue or GitHub might be unreachable.");
          print_info("You can manually add credentials to your .netrc file later.");
        }
      }
    }
  }

  // Set secure permissions on .netrc
  if netrc_path.exists() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let mut perms = fs::metadata(&netrc_path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&netrc_path, perms)?;

    print_success("Set secure permissions on .netrc file (600).");
  }

  println!();
  print_success("Credential setup complete!");
  print_info("You can now use twig with Jira and GitHub integration.");
  print_info(&format!(
    "Run {} to verify your credentials.",
    format_command("twig creds check")
  ));

  Ok(())
}
