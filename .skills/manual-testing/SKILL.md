---
name: manual-testing
description: >-
  Test twig CLI changes against a real Git repository. Use when verifying branch
  management features (tree, cascade, rebase, tidy, branch depend, sync, update,
  dashboard, switch) or when the user asks to test, try out, or validate twig
  behavior manually. Manages a dedicated sibling test repo at ../twig-test-dir.
---

# Manual Testing with twig-test-dir

Test twig changes against a real Git repository without risking any production
repos. The test repo lives at `d:\code\twig-test-dir`, a sibling to the main
twig workspace.

## Prerequisites

Before testing, build twig so the binary reflects your latest changes:

```
cargo build -p twig
```

The installed `twig` on PATH (`~/.cargo/bin/twig`) may be stale. For testing
in-progress changes, either:
- Run `cargo install --path .` to update the PATH binary, **or**
- Use the debug binary directly: `d:\code\twig\target\debug\twig.exe`

## Test repo location

```
d:\code\twig-test-dir
```

> **Note:** Adjust this path to match your workspace layout if it differs.

All twig commands must be run **from inside** this directory. Always `cd` (or
`Push-Location`) into it before executing twig commands.

## Setting up the test repo

### From scratch (if it doesn't exist)

```powershell
$testDir = "d:\code\twig-test-dir"
New-Item -ItemType Directory -Path $testDir -Force | Out-Null
Set-Location $testDir

git init
git config user.name "Test User"
git config user.email "test@example.com"

# Create initial commit on main
"# twig-test-dir" | Out-File -FilePath "README.md" -Encoding UTF8
git add README.md
git commit -m "Initial commit"
git branch -M main
```

### Resetting to a clean state

When the test repo gets messy, reset it completely:

```powershell
Set-Location "d:\code"
Remove-Item -Recurse -Force "d:\code\twig-test-dir"
# Then follow "From scratch" steps above
```

Or do a lighter reset that preserves the repo but cleans branches:

```powershell
Set-Location "d:\code\twig-test-dir"
git checkout main

# Delete all branches except main
git branch | Where-Object { $_ -notmatch '\* main' } | ForEach-Object {
    git branch -D $_.Trim()
}

# Clean twig state
Remove-Item -Recurse -Force ".twig" -ErrorAction SilentlyContinue
```

## Creating test branch trees

Most twig features operate on branch dependency trees. Here are recipes for
common test setups.

### Simple parent → child chain

```powershell
Set-Location "d:\code\twig-test-dir"

git checkout main

# Parent branch
git checkout -b feature-parent
"parent content" | Out-File -FilePath "parent.txt" -Encoding UTF8
git add parent.txt
git commit -m "Add parent feature"

# Child branch
git checkout -b feature-child
"child content" | Out-File -FilePath "child.txt" -Encoding UTF8
git add child.txt
git commit -m "Add child feature"

# Register dependencies in twig
twig branch root add main
twig branch depend feature-parent main
twig branch depend feature-child feature-parent

# Verify
twig tree
```

### Deep chain (A → B → C → D)

```powershell
Set-Location "d:\code\twig-test-dir"
git checkout main

$branches = @("branch-a", "branch-b", "branch-c", "branch-d")
$parent = "main"

twig branch root add main

foreach ($branch in $branches) {
    git checkout -b $branch
    "$branch content" | Out-File -FilePath "$branch.txt" -Encoding UTF8
    git add "$branch.txt"
    git commit -m "Commit for $branch"
    twig branch depend $branch $parent
    $parent = $branch
}

twig tree
```

### Diamond / multi-parent tree

```powershell
Set-Location "d:\code\twig-test-dir"
git checkout main

twig branch root add main

# Two parallel parents
git checkout main
git checkout -b parent-alpha
"alpha" | Out-File -FilePath "alpha.txt" -Encoding UTF8
git add alpha.txt; git commit -m "Alpha feature"
twig branch depend parent-alpha main

git checkout main
git checkout -b parent-beta
"beta" | Out-File -FilePath "beta.txt" -Encoding UTF8
git add beta.txt; git commit -m "Beta feature"
twig branch depend parent-beta main

# Child that depends on alpha
git checkout parent-alpha
git checkout -b child-branch
"child" | Out-File -FilePath "child.txt" -Encoding UTF8
git add child.txt; git commit -m "Child feature"
twig branch depend child-branch parent-alpha

twig tree
```

## Common testing workflows

### Testing `twig tree`

```powershell
Set-Location "d:\code\twig-test-dir"
twig tree              # Default view
twig tree --max-depth 2    # Limit depth
twig tree --no-color       # Without ANSI colors
```

### Testing `twig cascade`

```powershell
Set-Location "d:\code\twig-test-dir"

# Add a commit to a parent, then cascade changes down
git checkout feature-parent
"updated parent" | Out-File -FilePath "parent.txt" -Encoding UTF8
git add parent.txt
git commit -m "Update parent"

git checkout feature-parent
twig cascade
```

### Testing `twig rebase`

```powershell
Set-Location "d:\code\twig-test-dir"
git checkout feature-child
twig rebase
```

### Testing `twig tidy`

```powershell
Set-Location "d:\code\twig-test-dir"

# Create an empty branch (no unique commits) for clean-up
git checkout main
git checkout -b empty-branch
twig branch depend empty-branch main

twig tidy --dry-run    # Preview what would be cleaned
twig tidy              # Actually clean
```

### Testing `twig sync`

```powershell
Set-Location "d:\code\twig-test-dir"
twig sync
```

### Testing `twig switch`

```powershell
Set-Location "d:\code\twig-test-dir"
twig switch feature-parent
twig switch feature-child
```

## Verifying state

After running twig commands, verify the repo state:

```powershell
# Twig's view
twig tree

# Git's view
git log --oneline --graph --all | Select-Object -First 30
git branch -vv

# Twig internal state
Get-Content ".twig\state.json" | ConvertFrom-Json | ConvertTo-Json -Depth 10
```

## Tips

- **Always build first**: Run `cargo build -p twig` after code changes before
  testing.
- **Use debug binary for certainty**: `d:\code\twig\target\debug\twig.exe`
  guarantees you're running your local build.
- **Isolate tests**: Reset the repo between unrelated test scenarios to avoid
  leftover state confusing results.
- **Check exit codes**: `$LASTEXITCODE` after each twig command to catch silent
  failures.
- **Verbose output**: Add `-v`, `-vv`, or `-vvv` to any twig command for
  tracing output to diagnose issues.
- **Don't push**: The test repo has no remote — that's intentional. It's purely
  local.
