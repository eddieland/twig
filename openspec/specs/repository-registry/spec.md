# Repository Registry

## Purpose

Track and manage multiple git repositories in a global registry. Enables cross-repo operations like fetching all repos,
executing git commands across repos, and maintaining a central inventory of projects the user works with.

**CLI surface:** `twig git add/remove/list/exec/fetch`, flags: `-a/--all`, `-r/--repo` **Crates:** `twig-core`
(state::Registry), `twig-cli` (git command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
