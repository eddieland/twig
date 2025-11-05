# Migrating from argit to twig

This guide helps users of argit transition to twig, a new Git-based developer productivity tool with similar functionality but enhanced features and performance.

## Overview

Both argit and twig are tools designed to enhance Git workflows by providing branch dependency management, integration with external services, and visualization of branch relationships. While they share many conceptual similarities, twig is implemented in Rust (rather than Python) and offers additional features and performance improvements.

### Key Differences

| Feature            | argit                                      | twig                                        |
| ------------------ | ------------------------------------------ | ------------------------------------------- |
| **Implementation** | Python                                     | Rust                                        |
| **Architecture**   | Monolithic Python CLI app built with Click | Modular Rust workspace, CLI built with Clap |
| **Distribution**   | Internal PyPI repository                   | GitHub releases with pre-built binaries     |
| **Performance**    | ~500ms+ startup time (even with .pyc)      | Sub-100ms startup time                      |

### Performance Benefits

One of the most noticeable improvements when migrating from argit to twig is the dramatic reduction in command latency. The Rust implementation provides significantly faster startup times compared to Python:

- **CLI Help Response**: `twig --help` responds in **fractions of a second** (typically <100ms), while `argit --help` takes a **minimum of 500ms** even with pre-compiled `.pyc` files
- **Command Execution**: All twig commands benefit from near-instantaneous startup, eliminating the Python interpreter overhead that affects every argit command
- **Interactive Workflows**: The reduced latency makes twig feel more responsive during rapid command sequences and interactive workflows
- **Shell Completion**: Tab completion is noticeably faster, providing immediate feedback without the delay experienced with Python-based tools

This performance improvement becomes especially apparent during daily development workflows where you might run dozens of Git-related commands throughout the day.

### Command Mapping

Here's how argit commands map to their twig equivalents:

| argit command           | twig command                    | Notes                                                                                        |
| ----------------------- | ------------------------------- | -------------------------------------------------------------------------------------------- |
| `argit flow`            | `twig tree`                     | Shows branch relationships                                                                   |
| `argit flow <issue>`    | `twig switch <issue>`           | Creates/switches to branch for issue                                                         |
| `argit cascade`         | `twig cascade`                  | Rebases child branches                                                                       |
| No equivalent           | `twig rebase`                   | Rebases current branch on its ancestors (upward rebasing)                                    |
| `argit flow --root`     | `twig switch --root`            | Switch to current branch's root                                                              |
| `argit tidy`            | `twig git stale-branches`       | ⚠️ Partial equivalent - use `--prune` for interactive cleanup (e.g., `--days 30 --prune`)    |
| `argit ignore <branch>` | `twig branch root add <branch>` | Mark branches as special; a direct equivalent to `argit ignore` might be added in the future |

## Migration Steps

### 1. Install twig

Download and install twig from the [GitHub Releases](https://github.com/eddieland/twig/releases) page.

### 2. Initialize twig

```bash
twig init
```

This creates the necessary configuration files in your home directory.

### 3. Add your repositories

Unlike argit which is initialized per repository, twig tracks repositories globally:

```bash
# Add each repository you want to track
twig git add /path/to/repo1
twig git add /path/to/repo2
```

### 4. Set up credentials

twig uses the same `.netrc` file format as argit for credentials:

```bash
# Check if your credentials are properly configured
twig creds check

# If not, set them up
twig creds setup
```

### 5. Sync branch metadata

Since twig doesn't import argit configuration, you'll need to sync branch metadata:

```bash
# Scan branches and link them to Jira issues and GitHub PRs
twig sync

# Verify the links were created correctly
twig dashboard
```

### 6. Define branch dependencies (if needed)

If you had custom branch dependencies in argit, you'll need to recreate them:

```bash
# Make branch2 depend on branch1
twig branch depend branch2 branch1

# Mark important branches as roots
twig branch root add main
twig branch root add develop
```

### 7. Verify your setup

```bash
# Show your branch tree with dependencies
twig tree

# Show comprehensive dashboard
twig dashboard
```

## Understanding Branch Dependencies

In argit, branch dependencies were always manually provided by the user, not inferred from Git history.

Twig follows a similar explicit model where you define the relationships between branches:

```bash
# Make a branch depend on another branch
twig branch depend feature/child-branch feature/parent-branch

# Remove a dependency
twig branch depend remove feature/child-branch feature/parent-branch

# List dependencies for a branch
twig branch depend list feature/child-branch
```

Root branches (like `main` or `develop`) need to be explicitly marked:

```bash
# Add a root branch
twig branch root add develop

# Remove a root branch
twig branch root remove develop

# List root branches
twig branch root list
```

This explicit model gives you more control over how your branch tree is visualized and how cascading operations work.

## Feature Comparison

### Branch Visualization

argit's `flow` command is similar to twig's `tree` command, but twig offers more options:

```bash
# Basic tree view (similar to argit flow)
twig tree

# Include remote branches
twig tree --include-remote

# Show orphaned branches
twig tree --show-orphaned
```

### Branch Switching

argit's ability to switch to branches by Jira issue is available in twig with enhanced capabilities:

```bash
# Switch to branch by Jira issue (like argit flow PROJ-123)
twig switch PROJ-123

# Switch by GitHub PR
twig switch 12345

# Switch by URL (Jira or GitHub)
twig switch https://jira.example.com/browse/PROJ-123
```

### Rebasing Operations

twig offers two complementary rebase commands that work in opposite directions:

#### 1. Cascade (Downward Rebasing)

`twig cascade` starts at the current branch and works downward to all child branches:

```bash
# Rebase all child branches on their parents, starting from current branch
twig cascade

# Cascade with options
twig cascade --dry-run
twig cascade --continue-on-error
```

This is similar to argit's cascade command, updating all descendants of the current branch.

#### 2. Rebase (Upward Rebasing)

`twig rebase` works in the opposite direction - it rebases the current branch on its ancestors:

```bash
# Rebase current branch on its immediate parent
twig rebase

# Rebase current branch starting from the root branch
twig rebase --from-root

# Dry run to see what would happen
twig rebase --dry-run
```

This provides more flexibility compared to argit's cascade-only approach, allowing you to update your current branch with changes from its ancestors before cascading changes to descendants.

### Worktree Management

twig has comprehensive worktree support:

```bash
# Create a worktree for a branch
twig worktree create feature/new-thing

# List worktrees
twig worktree list

# Clean up unused worktrees
twig worktree clean
```

## Shell Completion

Enable shell completion for twig, similar to argit's approach but with a different implementation:

```bash
# Generate completion script for your shell
twig self completion bash > ~/.twig-completion.bash
echo 'source ~/.twig-completion.bash' >> ~/.bashrc

# Or for zsh
twig self completion zsh > ~/.twig-completion.zsh
echo 'source ~/.twig-completion.zsh' >> ~/.zshrc

# Or for fish
twig self completion fish > ~/.config/fish/completions/twig.fish
```

Unlike argit's `eval` approach, twig generates a static completion script that you can inspect before sourcing.

## Dashboard Feature

The `twig dashboard` command provides a comprehensive view of your development context:

```bash
# Basic dashboard
twig dashboard

# Include remote branches
twig dashboard --include-remote

# Disable GitHub or Jira API requests
twig dashboard --no-github
twig dashboard --no-jira

# Simple view (branches only)
twig dashboard --simple
```

This command shows:

- Local branches with their current status
- Associated pull requests with review status
- Related Jira issues with their current state
- Branch relationships
