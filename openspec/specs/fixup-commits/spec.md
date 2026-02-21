# Fixup Commits

## Purpose

Interactively select from recent commits and create fixup commits that will be automatically squashed during the next
interactive rebase. Filters by author, date range, and existing fixup status. Supports vim-mode navigation for power
users.

**CLI surface:** `twig fixup` (alias `fix`), flags: `--limit`, `--days`, `--all-authors`, `--include-fixups`,
`--dry-run`, `--vim-mode` **Crates:** `twig-core` (git ops), `twig-cli` (fixup command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
