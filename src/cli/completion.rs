use anyhow::Result;
use clap::{Arg, Command};

use crate::completion::{generate_completions, parse_shell};

/// Build the completion command
pub fn build_completion_command() -> Command {
  Command::new("completion")
    .about("Generate shell completions")
    .long_about(
      "Generates shell completion scripts for twig commands.\n\n\
             This command generates completion scripts that provide tab completion for twig\n\
             commands and options in your shell. Supported shells include bash, zsh, and fish.",
    )
    .arg(
      Arg::new("shell")
        .help("Shell to generate completions for")
        .required(true)
        .value_parser(["bash", "zsh", "fish"]),
    )
}

/// Handle the completion command
pub fn handle_completion_command(completion_matches: &clap::ArgMatches) -> Result<()> {
  let shell_str = completion_matches.get_one::<String>("shell").unwrap();
  let shell = parse_shell(shell_str)?;
  generate_completions(shell)
}
