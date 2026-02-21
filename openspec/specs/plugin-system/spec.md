# Plugin System

## Purpose

Discover and execute external plugins named `twig-<name>` found on `$PATH`. Plugins receive context via environment
variables (`TWIG_CONFIG_DIR`, `TWIG_DATA_DIR`, `TWIG_CURRENT_REPO`, `TWIG_CURRENT_BRANCH`, `TWIG_VERSION`, etc.) and can
use twig-core as a library dependency.

**CLI surface:** `twig self plugins` (discovery), `twig <plugin-name>` (execution) **Crates:** `twig-core`
(plugin::PluginContext), `twig-cli` (external command dispatch)

## Requirements

<!-- Requirements will be seeded from existing behavior -->
