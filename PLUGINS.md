# Twig Plugin Development Guide

This guide explains how to create plugins for twig, extending its functionality with external commands.

## Overview

Twig plugins follow a kubectl/Docker-inspired model where:

- Plugins are executable files named `twig-<plugin-name>`
- They are discovered via `$PATH` lookup
- Built-in commands always take precedence over plugins
- Plugins receive context through environment variables
- Plugins can be implemented in any language

## Plugin Naming Convention

All plugins must be named `twig-<plugin-name>` where `<plugin-name>` is the command you want to add to twig.

Examples:

- `twig-deploy` → `twig deploy`
- `twig-backup` → `twig backup`
- `twig-lint` → `twig lint`

## Environment Variables

When twig executes a plugin, it sets the following environment variables:

- `TWIG_CONFIG_DIR`: Path to twig's configuration directory
- `TWIG_DATA_DIR`: Path to twig's data directory
- `TWIG_CURRENT_REPO`: Current repository path (if in a repo)
- `TWIG_CURRENT_BRANCH`: Current branch name (if in a repo)
- `TWIG_VERSION`: Version of twig core that invoked the plugin
- `TWIG_VERBOSITY`: Verbosity level (0-3) passed from twig's `-v` flags
- `TWIG_COLORS`: Color preference (`yes`, `no`, `auto`) passed from twig

## Command Line Arguments

All arguments after the plugin name are passed through unchanged to the plugin.

Example: `twig deploy --env prod --force` → plugin receives `["--env", "prod", "--force"]`

## Exit Codes

Plugins should use standard exit codes:

- `0`: Success
- `1`: General error
- `2`: Misuse of command (invalid arguments)
- `130`: Interrupted by user (Ctrl+C)

## Verbosity and Logging

Plugins should respect the verbosity level passed from twig to provide consistent logging behavior across the ecosystem.

### Verbosity Levels

The `TWIG_VERBOSITY` environment variable contains a number (0-3) indicating the desired verbosity:

- `0`: Default level - show only warnings and errors
- `1`: Info level - show informational messages, warnings, and errors
- `2`: Debug level - show debug messages and everything above
- `3+`: Trace level - show trace messages and everything above

### Implementation Guidelines

- **Rust plugins**: Use `tracing` or `log` crates and map verbosity levels to appropriate log levels
- **Python plugins**: Configure the `logging` module based on `TWIG_VERBOSITY`
- **Shell scripts**: Use conditional output functions that check the verbosity level

### Best Practices for Plugin Logging

1. **Respect the verbosity level**: Don't output debug information when verbosity is 0
1. **Use appropriate log levels**: Reserve INFO for important user-facing messages
1. **Send logs to stderr**: Keep stdout clean for structured output that might be parsed
1. **Include context**: Add plugin name or operation context to log messages
1. **Be consistent**: Follow the same logging patterns as twig core for familiar UX

See the example plugins in `examples/plugins/` for concrete implementations.

## Plugin State Management

Plugins should manage their own state separately from twig's core state:

### Recommended State Locations

- **Configuration**: `$TWIG_CONFIG_DIR/plugins/<plugin-name>/`
- **Data/Cache**: `$TWIG_DATA_DIR/plugins/<plugin-name>/`
- **Logs**: `$TWIG_DATA_DIR/plugins/<plugin-name>/logs/`

### State File Formats

- Use JSON for simple configuration
- Use TOML for complex configuration files
- Use SQLite for structured data that needs querying

## Language-Specific Implementation

### Rust Plugins

For Rust plugins, use the `twig-core` crate to access twig's configuration and utilities. When running plugins directly
in development (without the twig CLI), prefer `PluginContext::discover()` to reconstruct the expected environment
automatically.

```toml
[dependencies]
twig-core = { path = "path/to/twig-core" }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
```

```rust
use anyhow::Result;
use clap::Parser;
use twig_core::{PluginContext, plugin, print_success, print_error};

#[derive(Parser)]
#[command(name = "twig-deploy")]
#[command(about = "Deploy applications using twig context")]
struct Cli {
    // CLI arguments here
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let context = PluginContext::discover()?;

    // Get plugin directories (honors TWIG_* env vars when present, auto-discovers otherwise)
    let config_dir = context.plugin_config_dir("deploy");
    let data_dir = context.plugin_data_dir("deploy");

    // Check if in git repository
    if !plugin::in_git_repository() {
        print_error("Not in a git repository");
        std::process::exit(1);
    }

    // Get current context
    if let Some(repo) = context.current_repo {
        println!("Repository: {}", repo.display());
    }

    if let Some(branch) = context.current_branch {
        println!("Branch: {}", branch);
    }

    // Plugin logic here
    Ok(())
}
```

### Python Plugins

```python
#!/usr/bin/env python3
import os
import argparse
from pathlib import Path

def get_twig_config():
    """Get twig configuration from environment variables."""
    return {
        'config_dir': Path(os.environ.get('TWIG_CONFIG_DIR', '')),
        'data_dir': Path(os.environ.get('TWIG_DATA_DIR', '')),
        'current_repo': os.environ.get('TWIG_CURRENT_REPO'),
        'current_branch': os.environ.get('TWIG_CURRENT_BRANCH'),
        'verbosity': int(os.environ.get('TWIG_VERBOSITY', '0')),
    }

def main():
    parser = argparse.ArgumentParser(
        prog='twig-backup',
        description='Backup repositories using twig context'
    )
    # Add subcommands and options

    args = parser.parse_args()
    config = get_twig_config()

    # Plugin logic here
```

### Shell Script Plugins

```bash
#!/bin/bash
set -euo pipefail

# Access twig configuration
TWIG_CONFIG_DIR="${TWIG_CONFIG_DIR:-}"
TWIG_DATA_DIR="${TWIG_DATA_DIR:-}"
TWIG_CURRENT_REPO="${TWIG_CURRENT_REPO:-}"
TWIG_CURRENT_BRANCH="${TWIG_CURRENT_BRANCH:-}"
TWIG_VERBOSITY="${TWIG_VERBOSITY:-0}"

# Plugin directories
PLUGIN_CONFIG_DIR="$TWIG_CONFIG_DIR/plugins/sync"
PLUGIN_DATA_DIR="$TWIG_DATA_DIR/plugins/sync"

# Create plugin directories
mkdir -p "$PLUGIN_CONFIG_DIR"
mkdir -p "$PLUGIN_DATA_DIR"

# Plugin logic here
```

## CLI Interface Consistency

To maintain a consistent user experience, follow these guidelines:

### Command Structure

```bash
twig-<plugin-name> [GLOBAL_OPTIONS] <SUBCOMMAND> [SUBCOMMAND_OPTIONS] [ARGS]
```

### Global Options (Recommended)

- `--help, -h`: Show help information
- `--version, -V`: Show plugin version
- `--verbose, -v`: Increase verbosity (can be repeated)
- `--quiet, -q`: Suppress non-essential output
- `--color <WHEN>`: Control colored output (auto, always, never)

### Help Text Format

```
<plugin-name> <version>
<brief-description>

USAGE:
    twig <plugin-name> [OPTIONS] <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information
    -v, --verbose    Increase verbosity

SUBCOMMANDS:
    <subcommand>    <description>
    help            Print this message or help for subcommands
```

### Error Handling

- Use consistent error message format: `Error: <description>`
- Provide actionable error messages when possible
- Use appropriate exit codes
- Include suggestions for common mistakes

### Output Formatting

- Use colors consistently with twig's color scheme
- Respect the `--color` option and `NO_COLOR` environment variable
- Use emojis sparingly and consistently with twig's style
- Format tables and lists consistently

## Testing Your Plugin

1. **Build your plugin** (for compiled languages)
1. **Make it executable** and place it in your `$PATH`
1. **Test basic functionality**:
   ```bash
   twig your-plugin --help
   twig your-plugin --version
   ```
1. **Test with twig context**:
   ```bash
   cd /path/to/git/repo
   twig your-plugin subcommand
   ```
1. **Test error conditions**:
   - Run outside git repository
   - Invalid arguments
   - Missing dependencies

## Distribution

Plugins are distributed independently of twig core:

1. **Binary releases**: Provide pre-compiled binaries for common platforms
1. **Package managers**: Submit to language-specific package managers
1. **Source code**: Provide clear build instructions
1. **Documentation**: Include installation and usage instructions

## Examples

See the `examples/plugins/` directory for complete examples:

- `twig-deploy/`: Rust plugin using twig-core
- `twig-backup`: Python plugin
- `twig-sync`: Shell script plugin

## Best Practices

1. **Use twig-core for Rust plugins** to access configuration consistently
1. **Handle missing environment variables gracefully**
1. **Provide clear error messages** when not in a git repository
1. **Follow semantic versioning** for your plugin
1. **Document your plugin's requirements** and dependencies
1. **Test on multiple platforms** if distributing binaries
1. **Respect user preferences** for colors and verbosity
1. **Keep plugin state separate** from twig's core state
