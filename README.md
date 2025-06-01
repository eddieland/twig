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

The easiest way to install Twig is to download a pre-built binary from the [GitHub Releases](https://github.com/eddieland/twig/releases) page.

1. Navigate to the [latest release](https://github.com/eddieland/twig/releases/latest)
2. Download the appropriate binary for your platform:
   - `twig-linux-x86_64.tar.gz` for Linux
   - `twig-macos-x86_64.tar.gz` for macOS
3. Extract the archive and move the binary to a location in your PATH:

```bash
# Example for Linux/macOS
tar -xzf twig-*.tar.gz
chmod +x twig
sudo mv twig /usr/local/bin/
```

### For Developers

If you want to contribute to Twig or build it from source, please refer to the [CONTRIBUTING.md](CONTRIBUTING.md) file for detailed instructions on:

- Setting up the development environment
- Installing Rustup and the required toolchain
- Building from source
- Running tests with nextest
- Using the Makefile for common development tasks
- Working with snapshot tests

The project uses a specific nightly toolchain for development, which is automatically configured through the `rust-toolchain.toml` file.

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

## Environment Variables

Twig supports several environment variables to customize its behavior. For the best experience, we recommend setting these variables in your shell profile file (`.bashrc`, `.zshrc`, or equivalent).

### Jira Integration

#### JIRA_HOST

Specifies the URL of your Jira instance.

- **Default**: `https://eddieland.atlassian.net`
- **Example**: `export JIRA_HOST="https://your-company.atlassian.net"`

**Authentication**: When `JIRA_HOST` is set, Twig will look for credentials in your `.netrc` file matching this hostname first. If not found, it falls back to looking for `atlassian.net` credentials.

**API Requests**: All Jira API requests will be sent to this host, allowing you to:

- View issues: `twig jira issue view PROJ-123`
- Create branches from issues: `twig jira branch create PROJ-123`
- Transition issues: `twig jira issue transition PROJ-123 "In Progress"`
- List issues: `twig jira issue list --project PROJ`

We recommend setting this in your shell profile to ensure it's always available:

```bash
# Add to your .bashrc, .zshrc, or equivalent
export JIRA_HOST="https://your-company.atlassian.net"
```

### XDG Base Directory Specification

Twig follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html) for storing configuration and data files. You can customize these locations with the following variables:

#### XDG_CONFIG_HOME

Specifies the base directory for configuration files.

- **Default**: `$HOME/.config`
- **Example**: `export XDG_CONFIG_HOME="$HOME/.my-config"`

#### XDG_DATA_HOME

Specifies the base directory for data files.

- **Default**: `$HOME/.local/share`
- **Example**: `export XDG_DATA_HOME="$HOME/.my-data"`

#### XDG_CACHE_HOME

Specifies the base directory for cache files.

- **Default**: `$HOME/.cache`
- **Example**: `export XDG_CACHE_HOME="$HOME/.my-cache"`

## Development Resources

For information about development workflows, Makefile usage, and snapshot testing, please refer to the [CONTRIBUTING.md](CONTRIBUTING.md) file.
