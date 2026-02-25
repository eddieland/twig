---
name: stacking
description: >-
  Build and manage stacked branch workflows with twig. Use when creating branch
  stacks, rebasing branches onto their parents, updating stacks from upstream,
  committing with Jira context, creating fixup commits, or syncing branch
  metadata. Covers twig rebase, twig update, twig commit, twig fixup, twig sync,
  and twig dashboard.
---

# Stacked Branches

Stacked branches are a series of dependent branches where each builds on the
previous one. Twig makes this workflow manageable with explicit dependency
tracking, per-branch rebasing, and sync tooling.

## What is a branch stack?

A stack is a chain of branches where each depends on the one before it:

```
main
└── feature-api           # Base feature work
    └── feature-api-tests # Tests on top of API changes
        └── feature-api-docs  # Docs on top of tests
```

Each branch contains only its own incremental work. Rebasing propagates
upstream changes down the chain.

## Creating a stack

### Step by step

```
# Start from main
git checkout main

# Create the first branch in the stack
twig switch feature-api -p=main
# ... make commits ...

# Stack the next branch on top
twig switch feature-api-tests -p=feature-api
# ... make commits ...

# Stack another
twig switch feature-api-docs -p=feature-api-tests
# ... make commits ...
```

### Verify the stack

```
twig tree
```

Output:

```
main
└── feature-api
    └── feature-api-tests
        └── feature-api-docs
```

## Rebasing a single branch

Rebase the current branch onto its defined parent:

```
twig rebase
```

Options:

```
twig rebase --force          # Rebase even if up-to-date
twig rebase --autostash      # Auto-stash/pop uncommitted changes
twig rebase --show-graph     # Show dependency graph before rebasing
twig rebase --no-interactive # Fail on conflicts (CI mode)
twig rebase --skip-commits=<hash1>,<hash2>  # Exclude specific commits
```

## Updating from upstream

`twig update` performs a full refresh workflow:

1. Switches to the root branch (e.g., `main`)
2. Fetches from origin
3. Pulls latest commits
4. Runs `twig cascade` to rebase all descendants

```
twig update                  # Full update + cascade
twig update --no-cascade     # Just fetch/pull, skip cascade
twig update --autostash      # Stash uncommitted changes first
twig update --force-cascade  # Force rebase even if up-to-date
twig update --show-graph     # Show tree before cascading
```

Alias: `twig up`

## Committing with Jira context

If the branch is linked to a Jira issue, create commits pre-filled from the
issue:

```
twig commit                          # Message: "PROJ-123: Issue summary"
twig commit -m "custom message"      # Override the summary
twig commit -p "WIP:" -s "(draft)"   # Add prefix/suffix
twig commit --no-fixup               # Always create normal commit
```

If a recent commit has the same message, twig offers to create a fixup commit
instead.

## Interactive fixup commits

Stage changes, then use the interactive selector to pick a target commit:

```
git add -p                           # Stage changes
twig fixup                           # Interactive commit picker
twig fixup --limit 50 --days 14      # Customize search window
twig fixup --all-authors              # Include other authors' commits
twig fixup --dry-run                 # Preview without committing
twig fixup --vim-mode                # Vim-style modal interface
```

Alias: `twig fix`

Commits are scored by recency, authorship, and Jira association to surface the
most relevant candidates.

## Syncing branch metadata

Auto-detect and link Jira issues and GitHub PRs from branch names:

```
twig sync                    # Detect and link everything
twig sync --dry-run          # Preview what would change
twig sync --force            # Update existing associations
twig sync --no-jira          # Skip Jira detection
twig sync --no-github        # Skip GitHub detection
```

Detected patterns include:
- `PROJ-123/feature-name` → Jira issue PROJ-123
- `feature/PROJ-123-description` → Jira issue PROJ-123
- PR lookup via GitHub API by branch name

## Reviewing your work

The dashboard shows branches with their linked PRs and Jira issues:

```
twig dashboard               # Full dashboard (API calls)
twig dashboard --simple      # No API calls (branches only)
twig dashboard --no-github   # Skip GitHub
twig dashboard --no-jira     # Skip Jira
twig dashboard --mine        # Only your items
twig dashboard --recent      # Last 7 days only
twig dashboard -f json       # JSON output
```

Aliases: `twig dash`, `twig v`

## Typical stacking workflow

```
# 1. Start the stack
twig switch feature-base -p=main

# 2. Work and commit
git add -A && twig commit

# 3. Stack the next branch
twig switch feature-next -p=feature-base

# 4. Work and commit
git add -A && twig commit

# 5. When main updates, refresh the whole stack
twig update

# 6. Check status
twig tree
twig dashboard --simple

# 7. Clean up merged branches
twig tidy
```

## Tips

- **Always use `twig switch -p=<parent>`** when creating stacked branches to
  set dependencies automatically.
- **`twig rebase`** only rebases the current branch. Use **`twig cascade`** to
  propagate changes down the entire tree (see the cascading skill).
- **`twig update`** is the one-command way to sync with upstream and rebase
  everything.
- Use **`-v`** on any command for info-level tracing, **`-vv`** for debug.
