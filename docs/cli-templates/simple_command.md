# Simple Command Template

This document provides a template for implementing a simple command using the derive-based approach.

## Template Code

```rust
//! # Simple Command
//!
//! Description of the command and its functionality.

use anyhow::Result;
use clap::Parser;

use crate::cli::derive::DeriveCommand;

/// Command for [describe purpose]
///
/// Replace this doc comment with a description of your command.
/// This will be used as the help text.
#[derive(Parser)]
#[command(name = "command-name")] // Always set the command name explicitly
#[command(about = "Short description of the command")]
#[command(long_about = "Longer description of the command that provides more details.\n\n\
            Include information about what the command does, when to use it,\n\
            and any important considerations.")]
pub struct SimpleCommand {
    /// Description of this argument
    ///
    /// This doc comment will be used as the help text for this argument.
    #[arg(short, long, value_name = "VALUE")]
    pub some_argument: Option<String>,

    /// Description of this flag
    #[arg(long)]
    pub some_flag: bool,
}

impl DeriveCommand for SimpleCommand {
    fn execute(self) -> Result<()> {
        // Implement your command logic here
        println!("Executing simple command");

        // Access arguments like this:
        if let Some(arg_value) = self.some_argument {
            println!("Argument value: {}", arg_value);
        }

        if self.some_flag {
            println!("Flag is enabled");
        }

        Ok(())
    }
}

// For backward compatibility with the existing API
impl SimpleCommand {
    /// Creates a clap Command for this command
    pub fn command() -> clap::Command {
        Self::command_for_update()
    }

    /// Parses command line arguments and executes the command
    pub fn parse_and_execute() -> Result<()> {
        let cmd = Self::parse();
        cmd.execute()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        SimpleCommand::command().debug_assert();
    }
}
```

## Usage

1. Copy the template to a new file in the `twig-cli/src/cli/derive/` directory
2. Rename `SimpleCommand` to your command name
3. Update the command name, about text, and long about text
4. Add your command-specific arguments and flags
5. Implement the `execute` method with your command logic
6. Add tests for your command
7. Update the `mod.rs` file to export your command
8. Update the `commands.rs` file to register your command
