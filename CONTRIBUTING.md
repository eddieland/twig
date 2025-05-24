# Contributing to Twig

Thank you for your interest in contributing to Twig! This document provides guidelines and instructions for contributing to the project.

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

3. Build the project
   ```bash
   cargo build
   ```

4. Run tests
   ```bash
   cargo test
   ```

## Code Quality Standards

All contributions should pass the following checks:

- **Formatting**: Run `cargo fmt` to ensure your code follows the project's formatting standards
- **Linting**: Run `cargo clippy` to check for common mistakes and improve code quality
- **Testing**: Run `cargo test` to ensure all tests pass

These checks are also enforced by our CI pipeline.

## Project Structure

```
src/
├── main.rs           # CLI entry point
├── cli/              # Command definitions
│   ├── mod.rs
│   ├── git.rs        # (to be implemented)
│   ├── jira.rs       # (to be implemented)
│   └── github.rs     # (to be implemented)
├── config/           # Configuration management
├── state/            # State management
├── git/              # Git operations
├── utils/            # Shared utilities
```

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

## Iterative Development

The project follows an iterative development approach as outlined in the project plan. Each iteration builds on the previous one and provides immediate value.