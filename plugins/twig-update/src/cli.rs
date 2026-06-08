use clap::Parser;
use twig_core::output::cli_styles;

#[derive(Parser, Debug)]
#[command(
  name = "twig-update",
  version = env!("CARGO_PKG_VERSION"),
  about = "Switch to root branch, fetch/pull from origin, and cascade-rebase all descendants",
  long_about = "Update the repository by switching to the root branch, fetching from origin,\n\
pulling the latest commits, and optionally running a cascading rebase to update\n\
all dependent branches. This is the one-command way to sync with upstream and\n\
propagate changes through your branch tree.",
  styles = cli_styles(),
)]
pub struct Cli {
  /// Path to a specific repository
  #[arg(short, long, value_name = "PATH")]
  pub repo: Option<String>,

  /// Skip the cascade operation after updating
  #[arg(long)]
  pub no_cascade: bool,

  /// Force cascade even if branches are up-to-date
  #[arg(long)]
  pub force_cascade: bool,

  /// Show dependency graph before cascading
  #[arg(long = "show-graph")]
  pub show_graph: bool,

  /// Automatically stash and pop pending changes
  #[arg(long)]
  pub autostash: bool,
}
