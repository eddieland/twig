//! # Self Command
//!
//! Implements the `twig self` command group for managing Twig's
//! own lifecycle behaviours like self-updating.

use anyhow::Result;
use clap::{Args, Subcommand};
use twig_core::output::{format_command, print_header, print_info, print_warning};

use super::completion;
use crate::self_update::{PluginInstallOptions, SelfUpdateOptions, run as run_self_update, run_plugin_install};
use crate::{diagnostics, plugin};

/// Arguments for the top-level `twig self` command.
#[derive(Args)]
pub struct SelfArgs {
  /// Subcommands under `twig self`
  #[command(subcommand)]
  pub command: SelfSubcommand,
}

/// Subcommands available under `twig self`.
#[derive(Subcommand)]
pub enum SelfSubcommand {
  /// Update Twig or its plugins to the latest release
  #[command(
    long_about = "Download the latest Twig release from GitHub and replace the current executable.\n\n\
This command determines the platform-specific binary to download, verifies permissions,\n\
handles sudo elevation when required, and ensures that the running executable is swapped\n\
out safely once the update completes.\n\n\
Use `twig self update flow`, `twig self update prune`, or `twig self update mcp` to install or\n\
update individual plugins instead."
  )]
  #[command(alias = "upgrade")]
  Update(SelfUpdateArgs),

  /// Run system diagnostics
  #[command(
    long_about = "Runs comprehensive system diagnostics to check twig's configuration and dependencies.\n\n\
            This command checks system information, configuration directories, credentials,\n\
            git configuration, tracked repositories, and network connectivity. Use this\n\
            command to troubleshoot issues or verify that twig is properly configured."
  )]
  #[command(alias = "diag")]
  Diagnose,

  /// Generate shell completions
  #[command(long_about = "Generates shell completion scripts for twig commands.\n\n\
            This command generates completion scripts that provide tab completion for twig\n\
            commands and options in your shell. Supported shells include bash, zsh, and fish.")]
  Completion(completion::CompletionArgs),

  /// Discover available Twig plugins on your PATH
  #[command(
    long_about = "Searches your PATH for executables following the twig-<command> naming\n\
convention and prints the plugins that can be invoked. Use this command to verify that\n\
Twig can locate your installed plugins."
  )]
  #[command(alias = "list-plugins")]
  Plugins,
}

/// Arguments for `twig self update`.
#[derive(Args, Debug, Clone)]
pub struct SelfUpdateArgs {
  /// Reinstall even if the latest version is already installed
  #[arg(long)]
  pub force: bool,

  /// What to update
  #[command(subcommand)]
  pub target: Option<UpdateTarget>,
}

/// Target for `twig self update` subcommands.
#[derive(Subcommand, Debug, Clone)]
pub enum UpdateTarget {
  /// Install or update the Twig flow plugin
  #[command(
    long_about = "Download the latest Twig flow plugin release from GitHub and install it alongside\n\
the Twig executable so it can be discovered via your PATH. Use this when you want to\n\
install or update the built-in flow plugin binary."
  )]
  Flow,

  /// Install or update the Twig prune plugin
  #[command(
    long_about = "Download the latest Twig prune plugin release from GitHub and install it alongside\n\
the Twig executable so it can be discovered via your PATH. Use this when you want to\n\
install or update the prune plugin binary."
  )]
  Prune,

  /// Install or update the Twig MCP server
  #[command(
    long_about = "Download the latest Twig MCP server release from GitHub and install it alongside\n\
the Twig executable so it can be discovered via your PATH. Use this when you want to\n\
install or update the MCP server binary."
  )]
  Mcp,

  /// Install or update the Twig update plugin
  #[command(
    long_about = "Download the latest Twig update plugin release from GitHub and install it alongside\n\
the Twig executable so it can be discovered via your PATH. Use this when you want to\n\
install or update the update plugin binary."
  )]
  Update,
}

/// Execute a `twig self` command.
pub fn handle_self_command(args: SelfArgs) -> Result<()> {
  match args.command {
    SelfSubcommand::Update(cmd) => handle_update_command(cmd),
    SelfSubcommand::Diagnose => diagnostics::run_diagnostics(),
    SelfSubcommand::Completion(cmd) => completion::handle_completion_command(cmd),
    SelfSubcommand::Plugins => list_plugins(),
  }
}

fn handle_update_command(args: SelfUpdateArgs) -> Result<()> {
  let plugin_opts = PluginInstallOptions { force: args.force };
  match args.target {
    None => run_self_update(SelfUpdateOptions { force: args.force }),
    Some(UpdateTarget::Flow) => run_plugin_install("twig-flow", plugin_opts),
    Some(UpdateTarget::Prune) => run_plugin_install("twig-prune", plugin_opts),
    Some(UpdateTarget::Mcp) => run_plugin_install("twig-mcp", plugin_opts),
    Some(UpdateTarget::Update) => run_plugin_install("twig-update", plugin_opts),
  }
}

fn list_plugins() -> Result<()> {
  let plugins = plugin::list_available_plugins()?;

  if plugins.is_empty() {
    print_warning("No Twig plugins were found in your PATH.");
    print_info(&format!(
      "Add executables named {} to a directory on your PATH to enable plugins.",
      format_command("twig-<command>")
    ));
    return Ok(());
  }

  print_header("Available Twig plugins");

  for plugin in plugins {
    println!("  {}", format_command(&format!("twig-{}", plugin.name)));

    if let Some(primary) = plugin.paths.first() {
      println!("    Path: {}", primary.display());
    }

    if let Some(size_in_bytes) = plugin.size_in_bytes {
      println!("    Size: {}", format_file_size(size_in_bytes));
    }

    if plugin.paths.len() > 1 {
      println!("    Also found at:");
      for alternate in plugin.paths.iter().skip(1) {
        println!("      - {}", alternate.display());
      }
    }
  }

  Ok(())
}

fn format_file_size(bytes: u64) -> String {
  const KIB: f64 = 1024.0;
  let mut size = bytes as f64;
  let units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let mut unit_index = 0;

  while size >= KIB && unit_index < units.len() - 1 {
    size /= KIB;
    unit_index += 1;
  }

  if unit_index == 0 {
    format!("{} {}", bytes, units[unit_index])
  } else {
    format!("{size:.1} {}", units[unit_index])
  }
}
