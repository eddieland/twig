# Jira Integration

## Purpose

Work with Jira issues from the terminal. View issue details, create branches from issues, link existing branches to
issues, transition issues through workflow states, open issues in the browser, and configure Jira parsing modes.
Credentials sourced from ~/.netrc, host from JIRA_HOST env var or config.

**CLI surface:** `twig jira view/open/create-branch/link-branch/transition/config` **Crates:** `twig-jira` (client,
endpoints, models), `twig-core` (jira_parser, state, utils), `twig-cli` (jira command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
