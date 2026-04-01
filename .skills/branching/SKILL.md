______________________________________________________________________

## name: branching description: >- Manage branch dependencies, root branches, and tree visualization with twig. Use when creating branches, defining parent-child relationships, viewing the branch tree, switching branches, reparenting orphans, or cleaning up stale branch configuration. Covers twig branch, twig tree, twig switch, and twig tidy.

# Branch Management

Twig tracks explicit parent-child relationships between Git branches. Unlike Git's implicit commit ancestry, these are
user-defined dependency links stored in `.twig/state.json`.

## Core concepts

- **Root branches** — Top-level branches (e.g., `main`) that anchor the tree.
- **Dependencies** — Explicit parent→child relationships between branches.
- **Orphaned branches** — Branches with no defined dependency or root status.
- **Branch tree** — The visual representation of all relationships.

## Setting up a repository

Before using branch management, initialize twig and designate at least one root:

```
twig init
twig branch root add main
```

To set a default root (used by `twig update` and `twig switch --root`):

```
twig branch root add main --default
```

## Defining dependencies

Create a parent-child relationship:

```
twig branch depend <child> <parent>
```

Examples:

```
twig branch depend feature-auth main
twig branch depend feature-auth-tests feature-auth
twig branch depend feature-auth-ui feature-auth
```

This produces:

```
main
└── feature-auth
    ├── feature-auth-tests
    └── feature-auth-ui
```

## Removing dependencies

```
twig branch rm-dep <child> <parent>
```

## Viewing the tree

```
twig tree                  # Full tree with status indicators
twig tree --max-depth 2    # Limit depth
twig tree --no-color       # Without ANSI colors
```

The tree shows `[up-to-date]` or rebase status next to each branch.

## Creating branches (IMPORTANT)

**Every new branch must have a parent dependency.** Orphaned branches break tree visibility and cascade operations.
Always use `twig switch -p` when creating branches, or add a dependency immediately after with `twig branch depend`.

### Preferred: create with `-p`

```
twig switch feature-new -p                  # Parent = current branch
twig switch feature-new -p=main             # Parent = main
twig switch feature-new -p=feature-auth     # Parent = specific branch
```

### Alternative: create then depend

If a branch was created with plain `git checkout -b`:

```
git checkout -b feature-new
twig branch depend feature-new main                # Reparent to main
twig branch depend feature-new feature-auth        # Or to logical parent
```

### Choosing the right parent

- **Branching off main** → parent is `main`
- **Stacking on another feature** → parent is that feature branch
- **Not sure** → default to `main` and reparent later if needed

```
# Example: create a stack
twig switch feature-api -p=main
twig switch feature-api-tests -p=feature-api
twig switch feature-api-docs -p=feature-api-tests
```

### Fixing orphaned branches

If branches exist without dependencies, reparent them with `twig adopt`:

```
twig adopt                       # Auto-detect and attach orphans
twig adopt --parent main         # Attach all orphans to main
twig adopt -y                    # Skip confirmation prompt
```

Or to reparent a single branch explicitly:

```
twig branch depend orphaned-branch main
```

## Adopting orphaned branches

`twig adopt` re-parents orphaned branches by attaching them to a chosen parent. It always previews the proposed tree and
asks for confirmation before making changes.

```
twig adopt                                    # Auto-detect parents for orphans
twig adopt --mode default-root                # Attach all orphans to default root
twig adopt --parent main                      # Attach all orphans to a specific branch
twig adopt -y                                 # Confirm without prompting
twig adopt --max-depth 3                      # Limit preview tree depth
twig adopt --no-color                         # Disable color in preview
```

## Switching branches

`twig switch` accepts multiple input types:

```
twig switch feature-auth          # By branch name
twig switch PROJ-123              # By Jira issue key
twig switch 456                   # By GitHub PR number
twig switch --root                # Jump to dependency tree root
twig switch feature-new --no-create   # Don't auto-create
```

## Managing root branches

```
twig branch root add main              # Add root
twig branch root add main --default    # Add as default root
twig branch root list                  # List all roots
twig branch root remove develop        # Remove root
```

## Cleaning up stale branches

### twig tidy clean (default)

Removes branches with no unique commits and no children:

```
twig tidy                      # Same as twig tidy clean
twig tidy clean --dry-run      # Preview
twig tidy clean --force        # Skip confirmation
twig tidy clean --aggressive   # Reparent children of empty intermediates
```

Aggressive mode: if A→B→C and B has no unique commits, C gets reparented to A and B is deleted.

### twig tidy prune

Removes references to branches that no longer exist in Git:

```
twig tidy prune                # Clean stale refs
twig tidy prune --dry-run      # Preview
```

## State storage

Branch configuration is stored per-repository at `.twig/state.json`. This file tracks:

- Root branches (with optional default)
- Parent-child dependencies
- Branch metadata (Jira issues, GitHub PRs)

Use `twig branch` commands to modify this state — avoid editing the JSON directly.

## Command aliases

| Full command  | Alias     |
| ------------- | --------- |
| `twig branch` | `twig br` |
| `twig tree`   | `twig t`  |
| `twig switch` | `twig sw` |
