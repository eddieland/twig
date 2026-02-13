# Worktree Integration Revamp

## Purpose

- Make worktrees a first-class part of the twig workflow instead of an isolated side feature.
- Reduce friction so that reaching for a worktree is as easy as checking out a branch.
- Unify the mental model: "I want to work on branch X" should surface worktree options naturally rather than requiring the user to think about `twig worktree` as a separate concept.

### Non-goals

- Changing how Git worktrees work under the hood — we continue to use `git2` worktree APIs.
- Multi-repo worktree orchestration (e.g., creating worktrees across all registered repos at once).

## Current State Analysis

### What exists today

```
twig worktree (wt)
├── create <branch>   — Create worktree + branch if needed; places in <repo>-worktrees/<sanitized-branch>
├── list (ls)         — List worktrees with metadata
└── clean             — Prune stale worktree references
```

Additional integration: `twig jira create-branch --with-worktree` calls `create_worktree` internally.

### Problems identified

1. **Worktrees are siloed.** They live in their own command group and don't integrate with the commands you use most (`switch`, `tree`, `cascade`). You have to actively remember they exist.

2. **`twig switch` is worktree-unaware.** If you already have a worktree for branch X and run `twig switch X`, twig does a checkout instead of telling you about the worktree or helping you navigate to it.

3. **No navigation aid.** After `twig worktree create`, twig prints the path but doesn't help you get there. There's no `twig wt cd <branch>` or even `twig wt path <branch>` to script with.

4. **`twig tree` doesn't show worktree status.** When viewing your branch dependency graph, there's no indication of which branches have active worktrees — exactly the info that would remind you to use them.

5. **No targeted removal.** `twig wt clean` only prunes stale references (directory gone). There's no way to intentionally tear down a specific worktree when you're done with a branch.

6. **Worktree creation doesn't set up dependencies.** Unlike `twig switch -p`, creating a worktree for a new branch doesn't wire up the parent dependency, so you lose the stacked-PR workflow.

7. **Directory naming is opaque.** `feature/foo` → `<repo>-worktrees/feature-foo` — the slash-to-hyphen sanitization isn't obvious and can conflict if you have branches like `feature-foo` and `feature/foo`.

## Guiding Constraints

- Breaking changes to command names, flags, and group structure are acceptable.
- Must preserve backwards compatibility of `.twig/state.json` worktree data (or migrate it).
- Worktree features should degrade gracefully — if you never use worktrees, they shouldn't add noise.
- Shell integration (cd to worktree) is fundamentally limited by subprocess restrictions; the best we can do is emit a path for the user to consume.

## Target Capabilities

### 1. `twig switch --worktree` / `-w` flag

Integrate worktree creation directly into the primary branch-switching workflow:

```bash
# Create worktree for new branch (sets up dependency like -p does today)
twig switch -w feature/new-thing -p main

# Create worktree for existing branch
twig switch -w feature/existing

# If worktree already exists, print path and suggest cd
twig switch -w feature/existing
# → Worktree already exists at ~/repos/twig-worktrees/feature-existing
# → Run: cd ~/repos/twig-worktrees/feature-existing
```

This makes the creation path `twig switch -w` rather than `twig worktree create`, putting it right where the user's fingers already are.

### 2. `twig switch` worktree awareness (without `-w`)

When running `twig switch <branch>` (no `-w`), if a worktree already exists for that branch:

```
⚠ Branch 'feature/foo' has an active worktree at ~/repos/twig-worktrees/feature-foo
  Switching in this checkout instead. Use `cd ~/repos/twig-worktrees/feature-foo` to work there.
```

This is a non-blocking informational message — the checkout still happens. The goal is awareness, not gatekeeping.

### 3. Worktree indicators in `twig tree`

Annotate branches that have active worktrees in the tree visualization:

```
main
├── feature/auth [worktree: ~/repos/twig-worktrees/feature-auth]
│   └── feature/auth-tests
└── feature/payments
```

### 4. `twig wt path <branch>` — scriptable path output

Emit the worktree path for a branch, enabling shell integration:

```bash
cd $(twig wt path feature/foo)

# Or in a shell alias
alias twcd='cd $(twig wt path'
```

No formatting, no headers — just the raw path on stdout. Exit non-zero if no worktree exists.

### 5. `twig wt remove <branch>` — intentional teardown

Remove a specific worktree by branch name:

```bash
twig wt remove feature/done-with-this
# → Removed worktree for 'feature/done-with-this' at ~/repos/twig-worktrees/feature-done-with-this
```

Options:
- `--delete-branch` / `-d`: also delete the local branch after removing the worktree.
- Refuses to remove if there are uncommitted changes (override with `--force`).

### 6. Dependency integration on worktree creation

When `twig switch -w` or `twig wt create` creates a new branch, wire up the parent dependency just like `twig switch -p` does today. The `-p` / `--parent` flag should work with worktree creation.

### 7. Revised command surface

**Before (current):**
```
twig worktree create <branch>
twig worktree list
twig worktree clean
twig jira create-branch --with-worktree
```

**After (proposed):**
```
# Primary creation path (new)
twig switch -w <branch> [-p <parent>]

# Management commands (twig wt)
twig wt list              # Same as today, enhanced with more info
twig wt path <branch>     # New: scriptable path output
twig wt remove <branch>   # New: intentional teardown
twig wt clean             # Same as today

# Deprecated / removed
twig wt create <branch>   # Replaced by twig switch -w; keep as hidden alias for transition

# Jira integration (enhanced)
twig jira create-branch --with-worktree   # Still works, now uses same code path as switch -w
```

## Subagent Execution Plan

### Task Backlog

| Priority | Task | Definition of Done | Notes | Status |
| -------- | ---- | ------------------ | ----- | ------ |
| P0 | Add `-w`/`--worktree` flag to `twig switch` | `twig switch -w <branch>` creates a worktree (or reports existing one); works with `-p` for dependency setup | Refactor `create_worktree` to accept optional parent dependency. Reuse `resolve_branch_base` logic from switch. | |
| P0 | Add worktree awareness to `twig switch` (without `-w`) | When switching to a branch that has an existing worktree, print an info message with the worktree path | Query `RepoState::get_worktree()` after checkout. Non-blocking — checkout still proceeds. | |
| P0 | Implement `twig wt remove <branch>` | Can remove a specific worktree by branch name; validates no uncommitted changes; `--force` overrides; `--delete-branch` option | Use `git2` worktree prune + fs removal. Update `RepoState`. | |
| P1 | Implement `twig wt path <branch>` | Outputs raw worktree path to stdout; exit 1 if none exists | Simple lookup in `RepoState` + validate path still exists on disk. | |
| P1 | Add worktree indicators to `twig tree` | Branches with active worktrees show an annotation in tree output | Extend `UserDefinedDependencyResolver` or tree renderer to include worktree info. | |
| P1 | Deprecate `twig wt create` in favor of `twig switch -w` | `twig wt create` still works but prints deprecation notice pointing to `twig switch -w` | Keep as hidden alias for backwards compat. | |
| P2 | Update `twig jira create-branch -w` to use unified code path | Jira create-branch worktree mode uses the same logic as `twig switch -w` | Reduces code duplication in jira.rs. | |
| P2 | Improve worktree directory naming | Use branch name directly (preserving `/` as directory separators) instead of sanitizing to hyphens; e.g., `<repo>-worktrees/feature/foo/` | Evaluate git2 constraints — worktree names may not support `/`. If so, keep sanitized name as internal identifier but use full branch name for directory structure. | |
| P3 | Shell integration helpers | Document shell aliases/functions for `cd $(twig wt path ...)` patterns; consider a `twig wt shell <branch>` that spawns a subshell in the worktree directory | Subprocess can't change parent shell's cwd; document the workaround patterns. | |

### Risks & Mitigations

- **Risk:** `twig switch -w` overloads `switch` with too many concerns. **Mitigation:** Keep the implementation modular — `-w` calls into the same `create_worktree` core function, just with dependency wiring added. The switch command dispatches early based on the flag.

- **Risk:** Worktree path validation may diverge from git state (stale entries in state.json). **Mitigation:** `twig wt list` and `twig wt path` should validate paths exist on disk and warn about stale entries. `twig wt clean` remains for batch cleanup.

- **Risk:** Directory naming change (P2) could break existing worktree layouts. **Mitigation:** Only apply new naming to newly created worktrees; existing entries remain valid via state.json path field.

### Open Questions

- Should `twig switch -w` be the *only* way to create worktrees, or should `twig wt create` remain as a non-deprecated alternative? (Current proposal: deprecate `create`, keep as hidden alias.)
- Should `twig cascade` operate across worktrees (rebase branches that are checked out in worktrees)? This is technically possible since git rebase works on refs, not working directories, but could surprise users with changed worktree contents.
- When `twig switch` detects an existing worktree (without `-w`), should it *refuse* the checkout and force you to use the worktree? Or just inform? (Current proposal: inform only.)

## Status Tracking (to be updated by subagent)

- **Current focus:** _Planning phase — spec review._
- **Latest completed task:** _N/A_
- **Next up:** _P0: Add `--worktree` flag to `twig switch`._

## Lessons Learned (ongoing)

- _Worktree support was well-implemented technically but failed to integrate into the primary user workflow, making it invisible in practice. Feature adoption requires meeting users where they already are (in this case, `twig switch`)._
