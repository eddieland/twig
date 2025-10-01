# twig

[![CI](https://github.com/eddieland/twig/actions/workflows/ci.yml/badge.svg)](https://github.com/eddieland/twig/actions/workflows/ci.yml)
[![Latest Release](https://img.shields.io/github/v/release/eddieland/twig?display_name=tag&sort=semver)](https://github.com/eddieland/twig/releases/latest)

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
- **MSRV**: 1.88.0
- **Target Platforms**: Ubuntu 24.04 (primary), macOS (secondary), Windows (tertiary)

## Installation

### Pre-built Binaries (Recommended)

The easiest way to install Twig is to download a pre-built binary from the [GitHub Releases](https://github.com/eddieland/twig/releases) page.

1. Navigate to the [latest release](https://github.com/eddieland/twig/releases/latest)
2. Download the appropriate binary for your platform:
   - `twig-linux-x86_64-v*.tar.gz` for Linux
   - `twig-macos-x86_64-v*.tar.gz` for macOS
   - `twig-windows-x86_64-v*.zip` for Windows
3. Verify the download (recommended):
   ```bash
   # Verify checksum (Linux/macOS)
   sha256sum -c twig-*-v*.tar.gz.sha256
   ```
4. Extract and install:

**Linux/macOS:**
```bash
tar -xzf twig-*-v*.tar.gz
chmod +x twig
sudo mv twig /usr/local/bin/
```

**Windows:**
```powershell
# Extract the zip file, then move to a PATH location
Move-Item -Path twig.exe -Destination "$env:LOCALAPPDATA\Microsoft\WindowsApps\"
```

### Build from Source

If you want to build Twig from source:

```bash
# Clone the repository
git clone https://github.com/eddieland/twig.git
cd twig

# Build with Cargo
cargo build --release

# Install locally
cargo install --path twig-cli
```

**Requirements:**
- Rust 1.88.0 or later
- Git

### Verify Installation

```bash
twig --version
twig --help
```

### For Contributors

If you want to contribute to Twig, please refer to the [CONTRIBUTING.md](CONTRIBUTING.md) file for detailed development setup instructions.

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

## Aliases

```bash
# Top-level command aliases
alias tt='twig tree'           # Show branch tree
alias tsw='twig switch'        # Magic branch switching
alias td='twig dashboard'      # Show dashboard

# Git subcommand aliases
alias tgf='twig git fetch --all' # Fetch all repositories

# Worktree subcommand aliases
alias twl='twig worktree list'   # List worktrees

# Jira subcommand aliases
alias tjv='twig jira view'     # View Jira issue

# GitHub subcommand aliases
alias tgps='twig github pr status' # Check PR status
```

These aliases can significantly reduce typing and make common twig operations more convenient.

## Development Resources

For information about development workflows, Makefile usage, and snapshot testing, please refer to the [CONTRIBUTING.md](CONTRIBUTING.md) file.

## Windows Usage

While Twig primarily targets Linux and macOS, it can also be used on Windows with some considerations:

### File Path Handling

Windows uses different path normalization techniques compared to Unix-based systems, which can sometimes cause issues:

- Windows uses backslashes (`\`) as path separators, while Unix uses forward slashes (`/`)
- Windows paths may include drive letters (e.g., `C:\`)
- Case sensitivity differs between Windows (case-insensitive) and Unix (case-sensitive)

These differences can lead to unexpected behavior when working with paths across different platforms, especially in a Git context where repositories might be accessed from multiple operating systems.

### Troubleshooting Windows-Specific Issues

If you encounter issues when using Twig on Windows:

1. **Enable verbose logging**: Run commands with the `--verbose` flag to get more detailed output

   ```
   twig --verbose git list
   ```

2. **Provide crash reports**: If Twig crashes, it will generate a crash report. Please include this when reporting issues:

   ```
   # Location of crash reports
   %USERPROFILE%\.local\share\twig\crash-reports\
   ```

3. **Include panic dumps**: If you encounter a panic, the error message contains valuable information for debugging. Copy the entire output when reporting issues.

4. **Check path normalization**: If you're experiencing path-related issues, try using forward slashes even on Windows, as Git often works better with Unix-style paths.

Providing these details when reporting Windows-specific issues will help us identify and fix problems more effectively.
