# Rebase

## Purpose

Rebase the current branch onto its parent(s) as defined in the twig dependency tree. Respects the custom dependency
graph rather than relying solely on git's tracking branches, enabling correct rebasing in stacked PR workflows.

**CLI surface:** `twig rebase` (alias `rb`), flags: `--force`, `--show-graph`, `--autostash`, `-r` **Crates:**
`twig-core` (git ops, state), `twig-cli` (rebase command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
