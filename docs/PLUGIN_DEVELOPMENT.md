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

## Command Line Arguments

All arguments after the plugin name are passed through unchanged to the plugin.

Example: `twig deploy --env prod --force` → plugin receives `["--env", "prod", "--force"]`

## Exit Codes

Plugins should use standard exit codes:
- `0`: Success
- `1`: General error
- `2`: Misuse of command (invalid arguments)
- `130`: Interrupted by user (Ctrl+C)

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

For Rust plugins, use the `twig-core` crate to access twig's configuration and utilities:

```toml
[dependencies]
twig-core = { path = "path/to/twig-core" }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
```

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};
use twig_core::{plugin, print_success, print_error};

#[derive(Parser)]
#[command(name = "twig-deploy")]
#[command(about = "Deploy applications using twig context")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    Staging { /* ... */ },
    Production { /* ... */ },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Get plugin directories
    let config_dir = plugin::plugin_config_dir("deploy")?;
    let data_dir = plugin::plugin_data_dir("deploy")?;

    // Check if in git repository
    if !plugin::in_git_repository() {
        print_error("Not in a git repository");
        std::process::exit(1);
    }

    // Get current context
    if let Some(repo) = plugin::current_working_repo()? {
        println!("Repository: {}", repo.display());
    }

    if let Some(branch) = plugin::current_branch()? {
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
import sys
import argparse
from pathlib import Path

def get_twig_config():
    """Get twig configuration from environment variables."""
    return {
        'config_dir': Path(os.environ.get('TWIG_CONFIG_DIR', '')),
        'data_dir': Path(os.environ.get('TWIG_DATA_DIR', '')),
        'current_repo': os.environ.get('TWIG_CURRENT_REPO'),
        'current_branch': os.environ.get('TWIG_CURRENT_BRANCH'),
    }

def get_plugin_config_dir(plugin_name):
    """Get plugin-specific config directory."""
    config = get_twig_config()
    plugin_dir = config['config_dir'] / 'plugins' / plugin_name
    plugin_dir.mkdir(parents=True, exist_ok=True)
    return plugin_dir

def main():
    parser = argparse.ArgumentParser(
        prog='twig-backup',
        description='Backup repositories using twig context'
    )
    parser.add_argument('-v', '--verbose', action='count', default=0)
    # Add subcommands and options

    args = parser.parse_args()
    config = get_twig_config()

    # Plugin logic here
```

### Shell Script Plugins

```bash
#!/bin/bash
set -euo pipefail

# Plugin metadata
PLUGIN_NAME="twig-sync"
PLUGIN_VERSION="1.0.0"

# Access twig configuration
TWIG_CONFIG_DIR="${TWIG_CONFIG_DIR:-}"
TWIG_DATA_DIR="${TWIG_DATA_DIR:-}"
TWIG_CURRENT_REPO="${TWIG_CURRENT_REPO:-}"
TWIG_CURRENT_BRANCH="${TWIG_CURRENT_BRANCH:-}"

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
2. **Make it executable** and place it in your `$PATH`
3. **Test basic functionality**:
   ```bash
   twig your-plugin --help
   twig your-plugin --version
   ```
4. **Test with twig context**:
   ```bash
   cd /path/to/git/repo
   twig your-plugin subcommand
   ```
5. **Test error conditions**:
   - Run outside git repository
   - Invalid arguments
   - Missing dependencies

## Distribution

Plugins are distributed independently of twig core:

1. **Binary releases**: Provide pre-compiled binaries for common platforms
2. **Package managers**: Submit to language-specific package managers
3. **Source code**: Provide clear build instructions
4. **Documentation**: Include installation and usage instructions

## Examples

See the `examples/plugins/` directory for complete examples:
- `twig-deploy/`: Rust plugin using twig-core
- `twig-backup`: Python plugin
- `twig-sync`: Shell script plugin

## Best Practices

1. **Use twig-core for Rust plugins** to access configuration consistently
2. **Handle missing environment variables gracefully**
3. **Provide clear error messages** when not in a git repository
4. **Follow semantic versioning** for your plugin
5. **Document your plugin's requirements** and dependencies
6. **Test on multiple platforms** if distributing binaries
7. **Respect user preferences** for colors and verbosity
8. **Keep plugin state separate** from twig's core state
