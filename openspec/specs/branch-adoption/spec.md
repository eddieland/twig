# Branch Adoption

## Purpose

Automatically detect orphaned branches (those without parent dependencies in the twig graph) and re-parent them to a
chosen parent branch. Supports auto, default-root, and explicit branch adoption modes with interactive preview and
confirmation.

**CLI surface:** `twig adopt`, flags: `--mode`, `--parent`, `-y/--yes`, `-d/--max-depth`, `--no-color` **Crates:**
`twig-core` (tree algorithms, state), `twig-cli` (adopt command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
