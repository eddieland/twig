# Twig

[![CI](https://github.com/eddieland/twig/actions/workflows/ci.yml/badge.svg)](https://github.com/eddieland/twig/actions/workflows/ci.yml)
[![Latest Release](https://img.shields.io/github/v/release/eddieland/twig?display_name=tag&sort=semver)](https://github.com/eddieland/twig/releases/latest)

Twig is a Git-first productivity tool that keeps branches, issues, and pull requests aligned across everything you are building. It was shaped around stacked pull-request workflows: make a change on one branch, and Twig will help you carry it forward to each dependent branch by rebasing in sequence. Multi-repo management, worktrees, and Jira or GitHub integrations sit on top of that core experience so you can keep related work in sync without babysitting each branch.

## Overview

Twig coordinates branch management, issue tracking, and review work across one or many repositories. The CLI standardizes branch naming, keeps metadata alongside each repo, and offers batch operations when you need to run the same Git command everywhere.

## Highlights

- Multi-repository management keeps related projects marching together.
- Worktree helpers make it easy to hop between branches without disturbing your main checkout.
- Jira integration links branches to issues and can transition cards as work progresses.
- GitHub integration surfaces pull-request status and review information alongside your local state.
- Batch commands let you fetch, execute shell commands, or check repository health in one go.
- Credential helpers set up API access using familiar `.netrc` entries.

## Platform notes

Twig is written in Rust (Edition 2024). The minimum supported Rust version is 1.91.0. We develop primarily on Ubuntu 24.04, with macOS and Windows builds available as well.

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

- Rust 1.91.0 or later
- Git

### Verify Installation

```bash
twig --version
twig --help
```

### Shell Completions

Twig provides tab completion for commands, flags, and dynamic values (branch names, Jira keys, PR IDs).

**Bash** (add to `~/.bashrc`):

```bash
source <(COMPLETE=bash twig)
```

**Zsh** (add to `~/.zshrc`):

```zsh
source <(COMPLETE=zsh twig)
```

**Fish**:

```fish
source (COMPLETE=fish twig | psub)
```

**PowerShell** (add to `$PROFILE`):

```powershell
COMPLETE=powershell twig | Out-String | Invoke-Expression
```

### For Contributors

If you want to contribute to Twig, please refer to the [CONTRIBUTING.md](CONTRIBUTING.md) file for detailed development setup instructions.

## Basic Usage

Initialize Twig in a working directory, then point it at the repositories you want to manage:

```bash
twig init
twig git add /path/to/repo
twig git list
```

Creating worktrees keeps your stacks tidy while you iterate:

```bash
twig worktree create feature/new-thing
twig worktree list
```

Batch commands help you stay current everywhere:

```bash
twig git fetch --all
twig github pr status
```

### Managing a stack of pull requests

When you open a Jira issue or decide to split a change across several branches, Twig can record the order of those branches and keep them synchronized. Suppose you start a branch from a Jira ticket and later build on top of it:

```bash
twig switch PROJ-123
twig commit

twig switch feature/new-ui
git commit -am "Implement UI"
```

If you need to update the base branch after review feedback, run:

```bash
twig cascade
```

Twig will walk the dependency chain, rebasing `feature/new-ui` on top of the refreshed PROJ-123 branch. Each pull request stays reviewable, and your local branches reflect the order you intended.

## Command Structure

```
twig
├── branch (br)             # Manage custom branch dependencies
│   ├── depend
│   ├── remove-dep
│   └── root
│       ├── add
│       ├── list (ls)
│       └── remove (rm)
├── cascade (casc)          # Rebase the current branch stack
├── commit                  # Create Jira-backed commits
├── creds                   # Credential management
│   ├── check
│   └── setup
├── fixup (fix)             # Interactive fixup commit selector
├── git (g)                 # Git repository registry
│   ├── add
│   ├── exec
│   ├── fetch
│   ├── list (ls)
│   ├── remove (rm)
│   └── stale-branches (stale)
├── github (gh)             # GitHub integration
│   ├── check
│   ├── checks (ci)
│   ├── open
│   └── pr
│       ├── link
│       ├── list (ls)
│       └── status (st)
├── init                    # Initialize Twig configuration
├── jira (j)                # Jira integration
│   ├── open
│   ├── create-branch
│   ├── link-branch
│   ├── transition
│   ├── view
│   └── config
├── rebase (rb)             # Rebase current branch onto its parents
├── self                    # Twig maintenance utilities
│   ├── update (upgrade)
│   ├── diagnose (diag)
│   ├── completion
│   └── plugins (list-plugins)
├── switch (sw)             # Intelligent branch switching
├── sync                    # Auto-link branches to Jira issues and PRs
├── tree (t)                # Visualize the dependency tree
└── worktree (wt)           # Git worktree management
    ├── clean
    ├── create (new)
    └── list (ls)
```

## Environment Variables

Twig supports several environment variables to customize its behavior. For the best experience, we recommend setting these variables in your shell profile file (`.bashrc`, `.zshrc`, or equivalent).

### Jira Integration

#### JIRA_HOST

Specifies the URL of your Jira instance.

- **Example**: `export JIRA_HOST="https://your-company.atlassian.net"`

**Authentication**: When `JIRA_HOST` is set, Twig will look for credentials in your `.netrc` file matching this hostname first. If not found, it falls back to looking for `atlassian.net` credentials.

**API Requests**: All Jira API requests will be sent to this host, allowing you to:

- View issues: `twig jira view PROJ-123`
- Create branches from issues: `twig jira create-branch PROJ-123`
- Transition issues: `twig jira transition PROJ-123 "In Progress"`
- Link branches: `twig jira link-branch PROJ-123 feature/some-work`

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

## MCP Server (Alpha)

> [!WARNING]
> The MCP server is in early alpha. APIs and tool schemas may change between releases.

Twig ships a second binary, `twig-mcp`, that exposes branch metadata, Jira issues, and GitHub PRs as a read-only [Model Context Protocol](https://modelcontextprotocol.io/) server. This lets AI coding agents query your repository context without shelling out to `twig` commands.

The server uses **stdio transport** and auto-detects the repository from its working directory (or use `--repo /path/to/repo` to override).

<details>
<summary><b>Install in Claude Code (CLI)</b></summary>

```sh
claude mcp add --scope user twig-mcp -- twig-mcp
```

Remove `--scope user` to install for the current project only.

To override the repository path:

```sh
claude mcp add --scope user twig-mcp -- twig-mcp --repo /path/to/repo
```

</details>

<details>
<summary><b>Install in Claude Desktop</b></summary>

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "twig-mcp": {
      "command": "twig-mcp",
      "args": []
    }
  }
}
```

To override the repository path, add `"args": ["--repo", "/path/to/repo"]`.

</details>

<details>
<summary><b>Install in Cursor</b></summary>

Add to `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (project-specific):

```json
{
  "mcpServers": {
    "twig-mcp": {
      "command": "twig-mcp",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>Install in VS Code / Copilot</b></summary>

Add to your VS Code `settings.json`:

```json
{
  "mcp": {
    "servers": {
      "twig-mcp": {
        "type": "stdio",
        "command": "twig-mcp",
        "args": []
      }
    }
  }
}
```

</details>

<details>
<summary><b>Install in Windsurf</b></summary>

Add to your Windsurf MCP config:

```json
{
  "mcpServers": {
    "twig-mcp": {
      "command": "twig-mcp",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>Install in Zed</b></summary>

Add to your Zed `settings.json`:

```json
{
  "context_servers": {
    "twig-mcp": {
      "command": {
        "path": "twig-mcp",
        "args": []
      }
    }
  }
}
```

</details>

<details>
<summary><b>Install in Cline</b></summary>

Add to your Cline MCP settings:

```json
{
  "mcpServers": {
    "twig-mcp": {
      "command": "twig-mcp",
      "args": []
    }
  }
}
```

</details>

### Available Tools

All tools are **read-only** and return structured JSON responses.

**Local State** — work without any API credentials:

| Tool                  | Description                                       |
| --------------------- | ------------------------------------------------- |
| `get_current_branch`  | Current branch name with linked Jira issue and PR |
| `get_branch_metadata` | Metadata for a specific branch                    |
| `get_branch_tree`     | Branch dependency tree visualization              |
| `get_branch_stack`    | Ancestor chain from a branch up to its root       |
| `list_branches`       | All twig-tracked branches in the repository       |
| `list_repositories`   | All twig-registered repositories                  |
| `get_worktrees`       | Active worktrees for the repository               |

**GitHub** — requires GitHub credentials in `~/.netrc` (see `twig creds setup`):

| Tool                 | Description                                       |
| -------------------- | ------------------------------------------------- |
| `get_pull_request`   | Full PR details (defaults to current branch's PR) |
| `get_pr_status`      | PR details with reviews and CI check status       |
| `list_pull_requests` | List PRs for the repository                       |

**Jira** — requires Jira credentials in `~/.netrc` and `JIRA_HOST` set:

| Tool               | Description                                               |
| ------------------ | --------------------------------------------------------- |
| `get_jira_issue`   | Issue details (defaults to current branch's linked issue) |
| `list_jira_issues` | List issues with project, status, and assignee filters    |

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
