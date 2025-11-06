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
  /// Render the branch tree rooted at the specified branch after switching to it.
  #[arg(long, value_name = "BRANCH")]
  pub root: Option<String>,

  /// Render the subtree for the specified parent branch after switching to it.
  #[arg(long, value_name = "BRANCH")]
  pub parent: Option<String>,

  /// Optional branch or ticket target for switching mode.
  #[arg(value_name = "TARGET")]
  pub target: Option<String>,
}
