# Initialization

## Purpose

Set up twig's local configuration files and directories for first-time use. Creates XDG-compliant config/data/cache
directories, initializes an empty global registry, and ensures the per-repo `.twig/` directory is git-ignored.

**CLI surface:** `twig init` **Crates:** `twig-core` (config::ConfigDirs, state), `twig-cli` (config command module)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
