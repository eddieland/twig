# Branch Dependencies

## Purpose

Manage custom parent/child relationships between branches and designate root branches that anchor dependency trees. This
is the foundational data model for twig's stacked workflow — all rebase, cascade, tree, and adoption features depend on
these relationships.

**CLI surface:** `twig branch depend`, `twig branch remove-dep`, `twig branch parent`,
`twig branch root add/list/remove` **Crates:** `twig-core` (state, graph), `twig-cli` (branch command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
