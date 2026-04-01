use clap::{ArgGroup, Parser};
use twig_core::cli_styles;

use crate::complete::flow_target_completer;

/// Command-line interface for the `twig flow` plugin.
#[derive(Debug, Parser, Clone)]
#[command(name = "twig-flow", about = "Branch visualization and switching for Twig workflows.")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(disable_help_subcommand = true)]
#[command(max_term_width = 120)]
#[command(styles = cli_styles())]
#[command(group(
  ArgGroup::new("tree_selection")
    .args(["root", "parent", "up", "down"])
    .multiple(false)
))]
pub struct Cli {
  /// Switch to the configured root branch before rendering the tree.
  #[arg(long)]
  pub root: bool,

  /// Switch to the current branch's parent before rendering.
  #[arg(long)]
  pub parent: bool,

  /// Switch to the previous branch in the tree (visually upward).
  #[arg(long)]
  pub up: bool,

  /// Switch to the next branch in the tree (visually downward).
  #[arg(long)]
  pub down: bool,

  /// Include branches whose names contain this pattern (case-insensitive;
  /// glob/regex may be added later).
  #[arg(long, value_name = "PATTERN")]
  pub include: Option<String>,

  /// Optional branch or ticket target for switching mode.
  #[arg(value_name = "TARGET", add = flow_target_completer())]
  pub target: Option<String>,
}
