# Commit Prefill

## Purpose

Create git commits with messages automatically prefilled from the Jira issue linked to the current branch. Formats
commit messages with the issue key and summary, with optional prefix/suffix customization. Detects duplicate commit
messages and offers fixup instead.

**CLI surface:** `twig commit`, flags: `-m/--message`, `-p/--prefix`, `-s/--suffix`, `--no-fixup` **Crates:**
`twig-core` (state, utils), `twig-jira` (issue fetch), `twig-cli` (commit command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
