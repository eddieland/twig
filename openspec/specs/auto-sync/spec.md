# Auto Sync

## Purpose

Scan branches and automatically detect and link them to Jira issues and GitHub PRs based on naming conventions and
remote tracking. Supports dry-run mode to preview changes, force mode to update existing associations, and selective
skipping of Jira or GitHub detection.

**CLI surface:** `twig sync`, flags: `--dry-run`, `--force`, `--no-jira`, `--no-github`, `-r` **Crates:** `twig-core`
(state, jira_parser, github), `twig-gh`, `twig-jira`, `twig-cli` (sync command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
