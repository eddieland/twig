# Contributing to Twig

This document provides guidelines and instructions for contributing to the project.

## Development Setup

1. Ensure you have Rust 1.87.0 or later installed
   ```bash
   rustup update
   ```

2. Clone the repository
   ```bash
   git clone <repository-url>
   cd twig
   ```

3. Install development tools
   ```bash
   make install-dev-tools
   ```

4. Set up pre-commit hooks
   ```bash
   make pre-commit-setup
   ```

5. Build the project
   ```bash
   cargo build
   ```

6. Run tests
   ```bash
   cargo test
   ```

## Code Quality Standards

All contributions should pass the following checks:

- **Formatting**: Run `cargo fmt` to ensure your code follows the project's formatting standards
- **Linting**: Run `cargo clippy` to check for common mistakes and improve code quality
- **Testing**: Run `cargo test` to ensure all tests pass

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

## Workflow

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
