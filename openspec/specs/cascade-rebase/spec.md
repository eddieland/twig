# Cascade Rebase

## Purpose

Rebase the current branch and then automatically cascade the rebase to all descendant branches in the dependency tree.
This is the core operation for maintaining stacked PRs — when a base branch changes, all branches built on top of it are
updated in topological order.

**CLI surface:** `twig cascade` (alias `casc`), flags: `--max-depth`, `--force`, `--show-graph`, `--autostash`,
`--preview`, `-r` **Crates:** `twig-core` (git ops, graph, state), `twig-cli` (cascade command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
