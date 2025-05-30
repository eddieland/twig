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
