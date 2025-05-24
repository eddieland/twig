# twig

A Git-based developer productivity tool that enhances workflows by integrating git repository management with Jira issue tracking and GitHub pull request workflows.

## Overview

Twig streamlines common developer workflows across multiple repositories, providing consistent branch naming and management, integrating issue tracking with code development, and enabling batch operations across tracked repositories.

## Key Features

- **Multi-repository management**: Track and perform operations across multiple git repositories
- **Worktree support**: Efficiently manage git worktrees for feature development
- **Jira integration**: Connect branches to Jira issues and manage transitions
- **GitHub integration**: Track PR status and review information
- **Batch operations**: Execute commands across all tracked repositories
- **Credential management**: Simplified setup for API access

## Technology Stack

- **Language**: Rust (Edition 2024)
- **MSRV**: 1.87.0
- **Target Platforms**: Ubuntu 24.04 (primary), macOS (secondary)

## Binary Size Optimization

Twig is optimized for minimal binary size through the following techniques:

- **Balanced optimization**: Uses `opt-level = 2` for good performance while keeping size reasonable
- **Link Time Optimization**: Enables LTO for cross-module optimizations
- **Reduced code generation units**: Uses single codegen unit for better optimization
- **Panic handling optimization**: Uses `panic = "abort"` to remove backtrace code
- **Symbol stripping**: Removes debug symbols from release builds
- **Dependency trimming**: Minimizes feature flags for dependencies like tokio

These optimizations ensure Twig remains lightweight (~a few MB in release mode) while maintaining full functionality.

## Installation

*Coming soon*

## Basic Usage

```bash
# Initialize twig
twig init

# Add repositories to track
twig git add /path/to/repo
twig git list

# Create a worktree for a feature branch
twig worktree create feature/new-thing

# Fetch all tracked repositories
twig git fetch --all

# Create a branch from a Jira issue
twig jira branch create PROJ-123

# Check PR status
twig github pr status
```

## Command Structure

```
twig
├── init                    # Initialize configuration
├── config (cfg)            # Configuration management
│   ├── show
│   ├── set
│   └── validate
├── git (g)                # Git repository management
│   ├── list (ls)
│   ├── add
│   ├── remove (rm)
│   ├── fetch
│   ├── exec
│   └── stale-branches (stale)
├── worktree (wt)          # Worktree management
│   ├── create (new)
│   ├── list (ls)
│   └── clean
├── jira (j)               # Jira integration
│   ├── issue (i)
│   │   ├── view (show)
│   │   └── transition (trans)
│   └── branch (br)
│       ├── create (new)
│       └── link
├── github (gh)            # GitHub integration
│   ├── pr
│   │   ├── status (st)
│   │   └── link
│   └── check
├── creds                  # Credential management
│   ├── check
│   └── setup
├── diagnose (diag)        # System diagnostics
├── update                 # Self-update
├── completion             # Shell completions
└── version                # Version information
```

## Makefile

The project includes a Makefile to simplify common development tasks. The Makefile is self-documenting and provides a helpful overview of available commands:

```bash
make help
```

Key Makefile targets include:

- **Development**: `fmt`, `lint`, `test`, `check`, `doc`
- **Build**: `build`, `release`, `clean`, `run`
- **Installation**: `install`, `install-dev-tools`, `pre-commit-setup`

## Contributing

Contributions are welcome! Here's how you can contribute to the project:

### Development Setup

1. Ensure you have Rust 1.87.0 or later installed
2. Clone the repository
3. Run `make install-dev-tools` to install development tools
4. Run `make pre-commit-setup` to set up pre-commit hooks
5. Run `cargo build` to build the project
6. Run `cargo test` to ensure everything is working correctly

### Code Quality

All contributions should pass the following checks:
- `cargo fmt` for code formatting
- `cargo clippy` for linting
- `cargo test` for unit and integration tests

These checks are automatically enforced by pre-commit hooks before each commit.

### Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Implementation Guidelines

- Follow the code organization structure in the project
- Add appropriate error handling using `anyhow`
- Write tests for new functionality
- Update documentation as needed
