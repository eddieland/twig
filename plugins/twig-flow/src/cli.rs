use clap::{ArgGroup, Parser};
use twig_core::cli_styles;

/// Command-line interface for the `twig flow` plugin.
#[derive(Debug, Parser, Clone)]
#[command(name = "twig-flow", about = "Branch visualization and switching for Twig workflows.")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(disable_help_subcommand = true)]
#[command(max_term_width = 120)]
#[command(styles = cli_styles())]
#[command(group(
  ArgGroup::new("tree_selection")
    .args(["root", "parent"])
    .multiple(false)
))]
pub struct Cli {
  /// Switch to the configured root branch before rendering the tree.
  #[arg(long)]
  pub root: bool,

  /// Switch to the current branch's parent before rendering.
  #[arg(long)]
  pub parent: bool,

  /// Include branches whose names contain this pattern (case-insensitive;
  /// glob/regex may be added later).
  #[arg(long, value_name = "PATTERN")]
  pub include: Option<String>,

  /// Optional branch or ticket target for switching mode.
  #[arg(value_name = "TARGET")]
  pub target: Option<String>,
}
