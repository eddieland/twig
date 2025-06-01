# Contributing to Twig

This document provides guidelines and instructions for contributing to the project.

## Development Setup

### Installing Rustup

[Rustup](https://rustup.rs/) is the official Rust toolchain installer that makes it easy to install Rust and switch between different versions.

1. **Linux/macOS**:

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Verify installation**:

   ```bash
   rustup --version
   cargo --version
   rustc --version
   ```

### Setting Up the Right Toolchain

Twig requires Rust 1.87.0 or later and uses the **nightly** toolchain for unstable rustfmt features. The project includes a `rust-toolchain.toml` file that specifies the exact requirements.

```bash
# Simply navigate to the project directory and Rustup will automatically detect the toolchain file
cd twig
rustup show
```

The `rust-toolchain.toml` file in the repository will ensure the correct toolchain is used when building the project.

### Building from Source

Once you have Rustup installed:

```bash
# Clone the repository
git clone https://github.com/eddieland/twig.git
cd twig

# Install development tools
make install-dev-tools

# Set up pre-commit hooks
make pre-commit-setup

# Build the project
cargo build

# Build in release mode
cargo build --release

# The binary will be available at target/release/twig
```

### Running Tests

```bash
make test
# OR
cargo nextest run
```

**Important**: This project uses [nextest](https://nexte.st/) for running tests instead of the standard `cargo test`. The test suite will only work correctly with nextest. Running `cargo test` directly will not execute tests properly.

## Code Quality Standards

All contributions should pass the following checks:

- **Formatting**: Run `cargo fmt` to ensure your code follows the project's formatting standards
- **Linting**: Run `cargo clippy` to check for common mistakes and improve code quality
- **Testing**: Run `make test` or `cargo nextest run` to ensure all tests pass

**Note**: This project requires [nextest](https://nexte.st/) for testing. The standard `cargo test` command will not work correctly.

These checks are automatically enforced by pre-commit hooks and our CI pipeline.

### Pre-commit Hooks

This project uses pre-commit hooks to ensure code quality standards are met before each commit. Pre-commit is installed using [uv tool](https://github.com/astral-sh/uv), which installs Python tools without requiring a virtual environment.

The hooks will:

- Check for trailing whitespace and fix it
- Ensure files end with a newline
- Validate YAML files
- Check for merge conflicts
- Run rustfmt to format Rust code
- Run clippy to lint Rust code

If a hook fails, the commit will be aborted. You can run the hooks manually with:

```bash
make pre-commit-run
```

To temporarily bypass the hooks (not recommended), use the `--no-verify` flag with git commit.

## Makefile

The project includes a Makefile to simplify common development tasks. The Makefile is self-documenting and provides a helpful overview of available commands:

```bash
make help
```

Key Makefile targets include:

- **Development**: `fmt`, `lint`, `test`, `check`, `doc`
- **Build**: `build`, `release`, `clean`, `run`
- **Installation**: `install`, `install-dev-tools`, `pre-commit-setup`
- **Snapshot Testing**: `insta-review`, `insta-accept`, `insta-reject`, `update-snapshots`

**Important**: All test-related targets use [nextest](https://nexte.st/) instead of the standard `cargo test`. This provides better performance and additional features for test execution.

## Snapshot Testing

Twig uses [Insta](https://insta.rs/) for snapshot testing, which helps ensure consistent output across changes. Snapshot tests capture the output of components and compare them against previously saved "snapshots" to detect unintended changes.

### Workflow

1. **Running Tests**: When you run tests with `make test`, any snapshot tests will be executed
2. **Reviewing Changes**: If snapshots change or new ones are created, use `make insta-review` to interactively review them
3. **Accepting Changes**: Accept all pending snapshots with `make insta-accept`
4. **Rejecting Changes**: Reject all pending snapshots with `make insta-reject`
5. **Updating Snapshots**: Run tests and automatically update snapshots with `make test-update-snapshots`

## Development Workflow

1. Create a feature branch (`git checkout -b feature/amazing-feature`)
2. Make your changes
3. Ensure all tests pass and code quality checks succeed
4. Commit your changes (`git commit -m 'Add some amazing feature'`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## Implementation Guidelines

- Follow the code organization structure in the project
- Add appropriate error handling using `anyhow`
- Write tests for new functionality
- Update documentation as needed
- Consider binary size implications when adding new dependencies
- Only include necessary features for dependencies (especially for tokio and reqwest)

## CLI Implementation Guidelines

Twig uses [Clap](https://docs.rs/clap/latest/clap/) with the Derive-style approach for defining the command-line interface. This section explains how to work with the CLI structure when adding or modifying commands.

### Benefits of Clap's Derive Pattern

The Derive-style approach offers several advantages over other CLI definition methods:

1. **Type Safety**: Command arguments are strongly typed, catching errors at compile time rather than runtime.

2. **Maintainability**: Command structure is defined declaratively alongside the data structures that hold the parsed values, making the code more maintainable and self-documenting.

3. **Automatic Help Generation**: Clap automatically generates comprehensive help text, usage information, and error messages based on the struct definitions and doc comments.

4. **Reduced Boilerplate**: Compared to the Builder pattern, Derive requires less code to define commands and arguments, leading to cleaner, more readable code.

5. **IDE Support**: The Derive approach works well with IDE features like code completion and refactoring tools.

6. **Validation**: Argument validation is handled through Clap's attribute system, keeping validation logic close to the argument definition.

7. **Extensibility**: The command structure can be easily extended with new subcommands or arguments without modifying existing code.

This approach aligns well with Rust's emphasis on type safety and declarative programming, making it the preferred choice for Twig's CLI implementation.

### CLI Architecture

The CLI is structured as follows:

1. The top-level `Cli` struct in `twig-cli/src/cli/mod.rs` defines global options and the command enum
2. The `Commands` enum defines all top-level subcommands
3. Each subcommand has its own module in `twig-cli/src/cli/` with argument structs and handler functions
4. Some commands have their own subcommands, creating a nested command structure

### Adding a New Command

To add a new top-level command:

1. Create a new module in `twig-cli/src/cli/` (e.g., `my_command.rs`)
2. Define the command's arguments using a struct with `#[derive(Args)]`
3. Add the command to the `Commands` enum in `twig-cli/src/cli/mod.rs`
4. Implement a handler function in your module
5. Add the handler to the match statement in `handle_cli()` in `twig-cli/src/cli/mod.rs`

Example:

```rust
// In twig-cli/src/cli/my_command.rs
use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct MyCommandArgs {
    /// Description of the argument
    #[arg(long, short)]
    pub some_arg: String,
}

pub fn handle_my_command(args: MyCommandArgs) -> Result<()> {
    // Implementation
    Ok(())
}

// In twig-cli/src/cli/mod.rs
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...

    /// My new command description
    #[command(long_about = "Detailed description of my command")]
    MyCommand(my_command::MyCommandArgs),
}

// In handle_cli() function
match cli.command {
    // ... existing matches ...
    Commands::MyCommand(args) => my_command::handle_my_command(args),
}
```

### Adding Subcommands

For commands with their own subcommands:

1. Define a subcommand enum with `#[derive(Subcommand)]`
2. Create argument structs for each subcommand
3. Use a nested match statement in your handler function

Example:

```rust
#[derive(Args)]
pub struct MyCommandArgs {
    #[command(subcommand)]
    pub subcommand: MySubcommands,
}

#[derive(Subcommand)]
pub enum MySubcommands {
    /// Subcommand description
    SubA(SubAArgs),

    /// Another subcommand
    SubB(SubBArgs),
}

#[derive(Args)]
pub struct SubAArgs {
    // Arguments for SubA
}

#[derive(Args)]
pub struct SubBArgs {
    // Arguments for SubB
}

pub fn handle_my_command(args: MyCommandArgs) -> Result<()> {
    match args.subcommand {
        MySubcommands::SubA(sub_args) => {
            // Handle SubA
        },
        MySubcommands::SubB(sub_args) => {
            // Handle SubB
        },
    }
}
```

### Command Attributes

Clap provides several attributes to customize commands:

- `#[command(about = "...")]` - Short description
- `#[command(long_about = "...")]` - Detailed description
- `#[command(alias = "...")]` - Command alias
- `#[arg(long, short)]` - Long and short option flags
- `#[arg(required = true)]` - Required argument
- `#[arg(default_value = "...")]` - Default value

See the [Clap documentation](https://docs.rs/clap/latest/clap/derive/index.html) for more attributes.

### Real-World Example: Branch Command

The `branch` command in Twig demonstrates nested subcommands with multiple levels:

```rust
// Top-level branch command
#[derive(Args)]
pub struct BranchArgs {
  #[command(subcommand)]
  pub subcommand: BranchSubcommands,
}

// First level of subcommands
#[derive(Subcommand)]
pub enum BranchSubcommands {
  Depend(DependCommand),
  RemoveDep(RemoveDepCommand),
  Root(RootCommand),
}

// Second level of subcommands (for Root)
#[derive(Args)]
pub struct RootCommand {
  #[command(subcommand)]
  pub subcommand: RootSubcommands,
}

#[derive(Subcommand)]
pub enum RootSubcommands {
  Add(RootAddCommand),
  Remove(RootRemoveCommand),
  List(RootListCommand),
}
```

This creates a command structure like:
- `twig branch depend <child> <parent>`
- `twig branch remove-dep <child> <parent>`
- `twig branch root add <branch>`
- `twig branch root remove <branch>`
- `twig branch root list`

### Command Execution Flow

The execution flow for commands follows this pattern:

1. `main.rs` parses CLI arguments with `Cli::parse()`
2. `handle_cli()` in `cli/mod.rs` matches the top-level command
3. Command-specific handler functions process the arguments
4. Handler functions return `Result<()>` with `anyhow` for error handling

### Testing Commands

When adding new commands, consider adding tests:

1. Unit tests for command handler logic
2. Integration tests for command execution
3. Snapshot tests for command output using Insta

For example, to test a new command:

```rust
#[test]
fn test_my_command() {
    // Set up test environment
    let temp_dir = tempdir().unwrap();
    let repo_path = temp_dir.path();

    // Execute command logic
    let args = MyCommandArgs { /* ... */ };
    let result = handle_my_command(args);

    // Assert expected outcomes
    assert!(result.is_ok());
    // Additional assertions...
}
```

## Test Utilities

Twig provides several test utilities in the `twig-test-utils` crate to simplify common testing patterns:

### Environment Utilities

- **`EnvTestGuard`**: Overrides XDG directories to use a temporary directory for testing
- **`HomeEnvTestGuard`**: Overrides the HOME directory to use a temporary directory
- **`ConfigDirsTestGuard`**: Provides a reusable configuration directory structure

### Git Repository Utilities

- **`GitRepoTestGuard`**: Creates a temporary git repository for testing
  - `GitRepoTestGuard::new()`: Creates a new git repository without changing the current directory
  - `GitRepoTestGuard::new_and_change_dir()`: Creates a new git repository and changes the current directory to it
  - `GitRepoTestGuard::change_dir()`: Changes the current directory to the git repository
  - `GitRepoTestGuard::restore_dir()`: Restores the original working directory

### Helper Functions

- **`setup_test_env()`**: Sets up a test environment with `TestEnv` and `TestConfigDirs`
- **`setup_test_env_with_init()`**: Sets up and initializes a test environment
- **`setup_test_env_with_registry()`**: Sets up a test environment with an empty registry

### Example: Testing with a Git Repository

```rust
#[test]
fn test_git_functionality() {
    // Create a temporary git repository and change to its directory
    let git_repo = GitRepoTestGuard::new_and_change_dir();

    // Test code that depends on being in a git repository
    let result = some_function_that_uses_git();

    // Assert expected outcomes
    assert!(result.is_ok());

    // GitRepoTestGuard will automatically restore the original directory when dropped
}
```

These utilities help ensure tests are isolated, deterministic, and don't interfere with the user's actual environment.

### RAII Pattern for Test Resource Management

The test utilities in Twig follow the RAII (Resource Acquisition Is Initialization) pattern, which is particularly well-suited for Rust's ownership model:

- **Resource management through ownership**: Resources (like temporary directories and environment variables) are acquired during initialization and automatically cleaned up when the guard object is dropped.
- **Automatic cleanup**: The `Drop` trait implementation ensures resources are properly released even if tests panic.
- **Scope-based lifetime**: Test resources exist precisely for the scope where they're needed and are automatically cleaned up when they go out of scope.

For example, when using `HomeEnvTestGuard`:

```rust
{
    // Create a guard that sets up a temporary HOME directory
    let home_guard = HomeEnvTestGuard::new();

    // Test code that uses HOME directory
    // ...

    // When home_guard goes out of scope, the Drop implementation
    // automatically restores the original HOME environment variable
}
```

This pattern eliminates the need for explicit cleanup code and ensures resources are properly managed even in the presence of errors or early returns.
