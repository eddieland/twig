use clap::{ArgGroup, Parser};

/// Command-line interface for the `twig flow` plugin.
#[derive(Debug, Parser, Clone)]
#[command(name = "twig-flow", about = "Branch visualization and switching for Twig workflows.")]
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

  /// Disable OSC 8 hyperlinks in the rendered table.
  #[arg(long)]
  pub no_links: bool,

  /// Optional branch or ticket target for switching mode.
  #[arg(value_name = "TARGET")]
  pub target: Option<String>,
}
