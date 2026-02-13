use clap::Parser;
use twig_core::output::cli_styles;

#[derive(Parser, Debug)]
#[command(
  name = "twig-prune",
  about = "Delete local branches whose GitHub PRs have been merged",
  styles = cli_styles(),
)]
pub struct Cli {
  /// Delete without prompting for each branch.
  ///
  /// This flag is intentionally long to prevent accidental use.
  /// Deleted branches cannot be easily recovered.
  #[arg(long = "yes-i-really-want-to-skip-prompts")]
  pub skip_prompts: bool,

  /// Show what would be deleted without actually deleting anything.
  #[arg(long, short = 'n')]
  pub dry_run: bool,
}
