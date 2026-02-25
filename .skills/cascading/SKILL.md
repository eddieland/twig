---
name: cascading
description: >-
  Cascade rebases across branch dependency trees with twig. Use when rebasing
  all descendants of a branch, propagating upstream changes through a branch
  tree, handling rebase conflicts during cascade, force-pushing after cascade,
  or skipping specific commits. Covers twig cascade and its options.
---

# Cascading Rebase

`twig cascade` rebases the current branch's descendants in dependency order,
propagating changes through the entire branch tree automatically.

## How it works

Given this tree:

```
main
└── feature-api           ← you are here
    ├── feature-api-tests
    │   └── feature-api-perf
    └── feature-api-docs
```

Running `twig cascade` from `feature-api` will:

1. Rebase `feature-api-tests` onto `feature-api`
2. Rebase `feature-api-perf` onto `feature-api-tests`
3. Rebase `feature-api-docs` onto `feature-api`

Branches are processed in topological order — a branch is only rebased after
all its parents have been rebased.

## Basic usage

```
git checkout feature-api
twig cascade
```

Alias: `twig casc`

## Options

```
twig cascade --force              # Rebase even if branches are up-to-date
twig cascade --force-push         # Force push to remote after each rebase
twig cascade --autostash          # Auto-stash/pop uncommitted changes
twig cascade --show-graph         # Display dependency graph before starting
twig cascade --max-depth 2        # Limit cascade depth
twig cascade --no-interactive     # Fail on conflicts (CI/script mode)
twig cascade --skip-commits=<hashes>  # Exclude specific commits
```

## Conflict handling

When a rebase conflict occurs during cascade:

### Interactive mode (default)

Twig pauses and drops you into the conflicted state. You can:

1. Resolve conflicts in your editor
2. `git add <resolved-files>`
3. `git rebase --continue`
4. Twig resumes the cascade automatically

Or abort:

1. `git rebase --abort`
2. The cascade stops; remaining branches are skipped

### Non-interactive mode (`--no-interactive`)

The cascade fails immediately on the first conflict. Use this in CI/CD or
scripts where interactive resolution isn't possible.

## Force pushing

After a cascade rebase, local branches have diverged from their remote tracking
branches. Use `--force-push` to update remotes:

```
twig cascade --force-push
```

**Warning**: This overwrites remote branch history. Only use on branches you
own or where force-push is expected (e.g., feature branch stacks with draft
PRs).

## Skipping commits

Exclude specific commits from the rebase:

```
# By comma-separated hashes
twig cascade --skip-commits=abc1234,def5678

# From a file (one hash per line)
twig cascade --skip-commits=skip-list.txt
```

Commit hashes must be 7-64 character hex strings.

## Limiting depth

Restrict how far down the tree the cascade reaches:

```
twig cascade --max-depth 1    # Only direct children
twig cascade --max-depth 2    # Children and grandchildren
```

Without `--max-depth`, cascade processes the entire subtree.

## Cascade vs rebase vs update

| Command | What it does |
|---|---|
| `twig rebase` | Rebases **only** the current branch onto its parent |
| `twig cascade` | Rebases **all descendants** of the current branch |
| `twig update` | Fetches upstream, pulls, then runs cascade from root |

### When to use each

- **Made changes to a parent branch?** → `twig cascade` from that branch
- **Need to sync one branch with its parent?** → `twig rebase`
- **Starting your day / syncing with remote?** → `twig update`

## Full workflow example

```
# 1. Pull latest main
git checkout main
git pull

# 2. Update your feature branch
git checkout feature-api
twig rebase

# 3. Make changes on feature-api
echo "new code" >> api.rs
git add -A && git commit -m "Update API"

# 4. Cascade the change to all descendants
twig cascade

# 5. Verify everything is clean
twig tree

# 6. Push everything (if using remotes)
twig cascade --force-push
```

## Visualizing before cascading

Use `--show-graph` to see the dependency tree before any rebases start:

```
twig cascade --show-graph
```

This renders the tree like `twig tree` and then proceeds with the cascade.

## Tips

- **Build before cascading**: If you have code changes, `cargo build -p twig`
  first to make sure the binary reflects your latest work.
- **Autostash is your friend**: `--autostash` prevents "dirty working tree"
  errors when you have uncommitted changes.
- **Check `$LASTEXITCODE`**: After cascade, check the exit code to confirm all
  branches rebased successfully.
- **Verbose mode**: `twig cascade -vv` shows detailed rebase progress for each
  branch.
