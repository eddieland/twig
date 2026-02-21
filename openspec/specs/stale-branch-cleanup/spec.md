# Stale Branch Cleanup

## Purpose

Detect branches that haven't been updated within a configurable time window and offer interactive pruning. Also powers
the twig-prune plugin which identifies branches with merged PRs or completed Jira issues for safe deletion, including
twig state cleanup.

**CLI surface:** `twig git stale-branches` (alias `stale`), flags: `-d/--days`, `-p/--prune`, `--json`, `-r` **Plugin:**
`twig-prune` (merged PR / completed issue detection) **Crates:** `twig-core` (state eviction, git ops), `twig-cli` (git
command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
