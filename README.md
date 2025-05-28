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

## Installation

### For Users

The easiest way to install Twig is to download a pre-built binary from the [GitHub Releases](https://github.com/omenien/twig/releases) page.

1. Navigate to the [latest release](https://github.com/omenien/twig/releases/latest)
2. Download the appropriate binary for your platform:
   - `twig-x86_64-unknown-linux-gnu.tar.gz` for Linux
   - `twig-x86_64-apple-darwin.tar.gz` for macOS
3. Extract the archive and move the binary to a location in your PATH:

```bash
# Example for Linux/macOS
tar -xzf twig-*.tar.gz
chmod +x twig
sudo mv twig /usr/local/bin/
```

### For Developers

If you want to contribute to Twig or build it from source, you'll need to set up Rustup and the nightly toolchain.

#### Installing Rustup

[Rustup](https://rustup.rs/) is the official Rust toolchain installer that makes it easy to install Rust and switch between different versions.

1. **Linux/macOS**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Windows**:
   Download and run [rustup-init.exe](https://win.rustup.rs/x86_64) from the official site.

3. **Verify installation**:
   ```bash
   rustup --version
   cargo --version
   rustc --version
   ```

#### Setting Up the Right Toolchain

Twig requires Rust 1.87.0 or later and uses the **nightly** toolchain for unstable rustfmt features. The project includes a `rust-toolchain.toml` file that specifies the exact requirements.

```bash
# Simply navigate to the project directory and Rustup will automatically detect the toolchain file
cd twig
rustup show
```

The `rust-toolchain.toml` file in the repository will ensure the correct toolchain is used when building the project.

#### Building from Source

Once you have Rustup installed:

```bash
# Clone the repository
git clone https://github.com/omenien/twig.git
cd twig

# Build in release mode
cargo build --release

# The binary will be available at target/release/twig
```

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
│   │   └── transition
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
- **Snapshot Testing**: `insta-review`, `insta-accept`, `insta-reject`, `test-update-snapshots`

## Snapshot Testing

Twig uses [Insta](https://insta.rs/) for snapshot testing, which helps ensure consistent output across changes. Snapshot tests capture the output of components and compare them against previously saved "snapshots" to detect unintended changes.

### Workflow

1. **Running Tests**: When you run tests with `make test`, any snapshot tests will be executed
2. **Reviewing Changes**: If snapshots change or new ones are created, use `make insta-review` to interactively review them
3. **Accepting Changes**: Accept all pending snapshots with `make insta-accept`
4. **Rejecting Changes**: Reject all pending snapshots with `make insta-reject`
5. **Updating Snapshots**: Run tests and automatically update snapshots with `make test-update-snapshots`
