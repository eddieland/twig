---
name: cascading
description: >-
  Cascade rebases across branch dependency trees with twig. Use when rebasing
  all descendants of a branch, propagating upstream changes through a branch
  tree, handling rebase conflicts during cascade, or previewing the rebase plan.
  Covers twig cascade and its options.
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
twig cascade --autostash          # Auto-stash/pop uncommitted changes
twig cascade --show-graph         # Display dependency graph before starting
twig cascade --max-depth 2        # Limit cascade depth
twig cascade --preview            # Show the rebase plan without executing
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

## Force pushing

After a cascade rebase, local branches have diverged from their remote tracking
branches. Push each rebased branch manually:

```powershell
git push --force-with-lease
```

Repeat for each branch in the stack. `--force-with-lease` is safer than
`--force` — it refuses to overwrite if someone else pushed since your last
fetch.

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

# 6. Push each branch (if using remotes)
git push --force-with-lease
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
