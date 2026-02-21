# Worktrees

## Purpose

Manage git worktrees for parallel multi-branch development. Create worktrees linked to branches, list active worktrees,
and clean up stale ones. Integrates with branch creation flows (e.g., `twig jira create-branch --with-worktree`).

**CLI surface:** `twig worktree create/list/clean` (alias `wt`) **Crates:** `twig-core` (state, git ops), `twig-cli`
(worktree command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
