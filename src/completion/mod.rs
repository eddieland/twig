use std::io;

use anyhow::Result;
use clap_complete::{Shell, generate};

use crate::utils::output::{print_error, print_info, print_success};

/// Generate shell completions for the specified shell
pub fn generate_completions(shell: Shell) -> Result<()> {
  let mut cmd = crate::cli::build_cli();
  let app_name = cmd.get_name().to_string();

  print_info(&format!("Generating {shell} completions...",));

  generate(shell, &mut cmd, app_name, &mut io::stdout());

  print_success(&format!("{shell} completions generated successfully!",));
  print_info("To use the completions, save the output to a file and source it in your shell profile.");

  match shell {
    Shell::Bash => {
      print_info("For bash, add this to your ~/.bashrc:");
      println!("  source <(twig completion bash)");
      print_info("Or save to a file:");
      println!("  twig completion bash > ~/.local/share/bash-completion/completions/twig");
    }
    Shell::Zsh => {
      print_info("For zsh, add this to your ~/.zshrc:");
      println!("  autoload -U compinit && compinit");
      println!("  source <(twig completion zsh)");
      print_info("Or save to a file in your fpath:");
      println!("  twig completion zsh > ~/.local/share/zsh/site-functions/_twig");
    }
    Shell::Fish => {
      print_info("For fish, save to a file:");
      println!("  twig completion fish > ~/.config/fish/completions/twig.fish");
    }
    _ => {
      print_info("Refer to your shell's documentation for how to use the completions.");
    }
  }

  Ok(())
}

/// Parse a shell string into a Shell enum
pub fn parse_shell(shell_str: &str) -> Result<Shell> {
  match shell_str.to_lowercase().as_str() {
    "bash" => Ok(Shell::Bash),
    "zsh" => Ok(Shell::Zsh),
    "fish" => Ok(Shell::Fish),
    _ => {
      print_error(&format!("Unsupported shell: {shell_str}",));
      print_info("Supported shells: bash, zsh, fish");
      Err(anyhow::anyhow!("Unsupported shell: {}", shell_str))
    }
  }
}
