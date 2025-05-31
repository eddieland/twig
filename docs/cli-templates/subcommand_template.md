# Subcommand Template

This document provides a template for implementing a command with subcommands using the derive-based approach.

## Template Code

```rust
//! # Command With Subcommands
//!
//! Description of the command and its functionality.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::cli::derive::DeriveCommand;

/// Template for a command with subcommands
///
/// Replace this doc comment with a description of your command.
/// This will be used as the help text.
#[derive(Parser)]
#[command(name = "command-name")] // Always set the command name explicitly
#[command(about = "Short description of the command")]
#[command(long_about = "Longer description of the command that provides more details.\n\n\
            Include information about what the command does, when to use it,\n\
            and any important considerations.")]
pub struct CommandWithSubcommands {
    /// Global flag for this command
    #[arg(long)]
    pub global_flag: bool,

    /// Global option for this command
    #[arg(short, long, value_name = "VALUE")]
    pub global_option: Option<String>,

    /// The subcommand to execute
    #[command(subcommand)]
    pub subcommand: SubCommands,
}

/// Subcommands for the main command
#[derive(Subcommand)]
pub enum SubCommands {
    /// First subcommand
    First(FirstSubCommand),

    /// Second subcommand
    Second(SecondSubCommand),
}

/// First subcommand implementation
#[derive(Parser)]
pub struct FirstSubCommand {
    /// An argument specific to the first subcommand
    #[arg(short, long)]
    pub first_arg: String,
}

/// Second subcommand implementation
#[derive(Parser)]
pub struct SecondSubCommand {
    /// An argument specific to the second subcommand
    #[arg(short, long)]
    pub second_arg: bool,
}

impl DeriveCommand for CommandWithSubcommands {
    fn execute(self) -> Result<()> {
        // Access global arguments
        if self.global_flag {
            println!("Global flag is enabled");
        }

        if let Some(option) = self.global_option {
            println!("Global option: {}", option);
        }

        // Handle subcommands
        match self.subcommand {
            SubCommands::First(cmd) => {
                println!("Executing first subcommand with arg: {}", cmd.first_arg);
                // Implement first subcommand logic here
                Ok(())
            }
            SubCommands::Second(cmd) => {
                if cmd.second_arg {
                    println!("Second subcommand with flag enabled");
                } else {
                    println!("Second subcommand with flag disabled");
                }
                // Implement second subcommand logic here
                Ok(())
            }
        }
    }
}

// For backward compatibility with the existing API
impl CommandWithSubcommands {
    /// Creates a clap Command for this command
    pub fn command() -> clap::Command {
        Self::command_for_update()
    }

    /// Parses command line arguments and executes the command
    pub fn parse_and_execute(matches: &clap::ArgMatches) -> Result<()> {
        // Extract global arguments
        let global_flag = matches.get_flag("global_flag");
        let global_option = matches.get_one::<String>("global_option").cloned();

        // Handle subcommands
        match matches.subcommand() {
            Some(("first", sub_matches)) => {
                let first_arg = sub_matches.get_one::<String>("first_arg")
                    .cloned()
                    .unwrap_or_default();

                let cmd = Self {
                    global_flag,
                    global_option,
                    subcommand: SubCommands::First(FirstSubCommand { first_arg }),
                };

                cmd.execute()
            }
            Some(("second", sub_matches)) => {
                let second_arg = sub_matches.get_flag("second_arg");

                let cmd = Self {
                    global_flag,
                    global_option,
                    subcommand: SubCommands::Second(SecondSubCommand { second_arg }),
                };

                cmd.execute()
            }
            _ => {
                // Handle case where no subcommand was provided
                println!("No subcommand specified");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        CommandWithSubcommands::command().debug_assert();
    }
}
```

## Usage

1. Copy the template to a new file in the `twig-cli/src/cli/derive/` directory
2. Rename `CommandWithSubcommands` to your command name
3. Update the command name, about text, and long about text
4. Define your subcommands in the `SubCommands` enum
5. Create structs for each subcommand with their specific arguments
6. Implement the `execute` method to handle the command logic for each subcommand
7. Add tests for your command and subcommands
8. Update the `mod.rs` file to export your command
9. Update the `commands.rs` file to register your command
