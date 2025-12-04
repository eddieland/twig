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

  /// Filter branches by name (ancestors stay visible for context).
  #[arg(long, value_name = "PATTERN")]
  pub filter: Option<String>,

  /// Optional branch or ticket target for switching mode.
  #[arg(value_name = "TARGET")]
  pub target: Option<String>,
}
