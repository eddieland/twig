# Credentials

## Purpose

Manage authentication credentials for GitHub and Jira. Check if credentials are properly configured and set them up
interactively. Supports ~/.netrc file format and platform-specific credential stores (Unix keyring, Windows Credential
Manager).

**CLI surface:** `twig creds check`, `twig creds setup` **Crates:** `twig-core` (creds module, netrc parser, platform
backends), `twig-cli` (creds command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
