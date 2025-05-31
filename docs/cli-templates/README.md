# Derive-based CLI Templates

This directory contains templates for implementing commands using the derive-based approach with Clap in the Twig codebase.

## Available Templates

1. [Simple Command Template](simple_command.md) - Template for implementing a basic command without subcommands
2. [Subcommand Template](subcommand_template.md) - Template for implementing a command with subcommands

## Best Practices

When implementing a new command using the derive-based approach, follow these best practices:

1. **Command Naming**: Always set the command name explicitly with `#[command(name = "command-name")]` to avoid conflicts with the default package name.

2. **Argument IDs**: Remember that the derive approach converts kebab-case CLI arguments (like `--max-depth`) to snake_case struct fields (like `max_depth`). Tests and code that access these fields must use the snake_case version.

3. **Documentation**: Use doc comments on struct fields for help text. This ensures that the help text is consistent between the code and the CLI.

4. **Grouping**: Group related arguments in nested structs when appropriate to keep the command definition clean and organized.

5. **Subcommands**: Use enums with `#[derive(Subcommand)]` for subcommands to leverage Clap's built-in subcommand handling.

6. **Testing**: Always include tests for your command using `CommandFactory::debug_assert()` to verify the command structure.

7. **Backward Compatibility**: Implement the `command()` and `parse_and_execute()` methods for backward compatibility with the existing API.

## Implementation Process

1. Create a new file in the `twig-cli/src/cli/derive/` directory for your command
2. Copy the appropriate template and customize it for your command
3. Implement the `execute` method with your command logic
4. Add tests for your command
5. Update the `mod.rs` file to export your command
6. Update the `commands.rs` file to register your command
7. Update the `mod.rs` file in the parent directory to import your command

## Example

See the existing implementations in the `twig-cli/src/cli/derive/` directory for examples of how to use these templates:

- `init.rs` - Simple command implementation
- `tree.rs` - Command with arguments implementation
- `panic.rs` - Hidden command implementation
