---
name: pull-requests
description: >-
  Create and manage pull requests for the twig project. Use when branching off
  main, committing changes, pushing, creating PRs with gh CLI, viewing PR status
  with twig github commands, or following the contribution workflow. Covers the
  full PR lifecycle from branch creation to merge.
---

# Pull Request Workflow

## Contributing workflow

The standard workflow for getting changes into twig:

### 1. Start from main

```powershell
git checkout main
git pull origin main
```

### 2. Create a feature branch

Branch naming convention: `feature/<descriptive-name>`

```powershell
git checkout -b feature/my-change
```

### 3. Make changes and commit

```powershell
git add -A
git commit -m "feat: description of the change"
```

Commit message prefixes:
- `feat:` — new feature
- `fix:` — bug fix
- `refactor:` — code restructuring
- `docs:` — documentation only
- `test:` — adding/updating tests
- `chore:` — maintenance, deps, CI

### 4. Push and create PR

```powershell
git push -u origin feature/my-change
gh pr create --title "feat: description" --body "Summary of changes" --base main
```

For longer PR descriptions, use a file:

```powershell
gh pr create --title "feat: description" --body-file pr-body.md --base main
```

### 5. After PR is merged, clean up

```powershell
git checkout main
git pull origin main
git branch -d feature/my-change
```

## Using twig's GitHub integration

Twig has built-in GitHub PR commands under `twig github` (alias: `twig gh`).

### Check authentication

```
twig github check
```

Verifies `.netrc` credentials are working and shows the authenticated user.

### Link a PR to the current branch

```
twig github pr link 338
twig github pr link https://github.com/eddieland/twig/pull/338
```

This stores the PR association in `.twig/state.json` so other commands can
reference it.

### View PR status

```
twig github pr status
```

Shows review status and check results for the current branch's linked PR.

### View CI checks

```
twig github checks           # Current branch's PR
twig github checks 338       # Specific PR number
```

Alias: `twig github ci`

### List PRs for the repo

```
twig github pr list                    # Open PRs (default)
twig github pr list --state closed     # Closed PRs
twig github pr list --state all        # All PRs
twig github pr list --limit 10         # Limit results
```

Alias: `twig github pr ls`

### Open PR in browser

```
twig github open              # Current branch's PR
twig github open 338          # Specific PR number
```

## Stacked PRs workflow

When working with branch stacks, each branch typically becomes its own PR:

```powershell
# Create the stack
twig switch feature-api -p=main
# ... work and commit ...
git push -u origin feature-api
gh pr create --title "feat: API layer" --base main

twig switch feature-api-tests -p=feature-api
# ... work and commit ...
git push -u origin feature-api-tests
gh pr create --title "test: API tests" --base feature-api

# Link PRs to branches for twig tracking
git checkout feature-api
twig github pr link <pr-number>
git checkout feature-api-tests
twig github pr link <pr-number>

# See everything together
twig dashboard
```

### Updating stacked PRs after rebase

After running `twig cascade`, branches have been rebased and need force-pushing:

```powershell
twig cascade --force-push
```

Or push individually:

```powershell
git checkout feature-api
git push --force-with-lease

git checkout feature-api-tests
git push --force-with-lease
```

### Auto-linking with sync

Instead of manually linking PRs, use sync to auto-detect them:

```
twig sync                    # Auto-link branches to PRs
twig sync --dry-run          # Preview what would be linked
```

## Dashboard view

See branches with their PRs and Jira issues in one view:

```
twig dashboard               # Full view with API calls
twig dashboard --simple      # Branches only, no API calls
twig dashboard -f json       # JSON output
```

Aliases: `twig dash`, `twig v`

## Pre-PR checklist

Before creating a PR, ensure:

```powershell
make fmt          # Format code
make lint         # Clippy with -D warnings
make test         # All tests pass (uses nextest)
```

Or run the full pipeline:

```powershell
make all          # fmt + lint + test + build + validate
```

## Command aliases

| Full command | Alias |
|---|---|
| `twig github` | `twig gh` |
| `twig github checks` | `twig github ci` |
| `twig github pr list` | `twig github pr ls` |
| `twig github pr status` | `twig github pr st` |
| `twig dashboard` | `twig dash` or `twig v` |
