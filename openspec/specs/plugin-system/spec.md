# Plugin System

## Purpose

Discover and execute external plugins named `twig-<name>` found on `$PATH`. Plugins receive context via environment
variables (`TWIG_CONFIG_DIR`, `TWIG_DATA_DIR`, `TWIG_CURRENT_REPO`, `TWIG_CURRENT_BRANCH`, `TWIG_VERSION`, etc.) and can
use twig-core as a library dependency.

**CLI surface:** `twig self plugins` (discovery), `twig <plugin-name>` (execution) **Crates:** `twig-core`
(plugin::PluginContext), `twig-cli` (external command dispatch)

## Plugin Discovery

### Requirement: Discovering plugins on PATH

#### Scenario: Scanning PATH for twig plugins

WHEN `list_available_plugins()` is called THEN it reads the `$PATH` environment variable AND iterates through each
directory in PATH order AND scans for executable files matching the `twig-*` naming pattern AND collects all matching
executables into a `BTreeMap` keyed by plugin name (ensuring sorted, deduplicated output) AND returns a
`Vec<PluginInfo>` in alphabetical order

#### Scenario: Non-existent PATH directories are skipped

WHEN a directory listed in `$PATH` does not exist THEN it is silently skipped AND discovery continues with the remaining
directories

#### Scenario: Non-executable files are skipped

WHEN a file matching `twig-*` is found but is not executable (on Unix: mode `& 0o111 == 0`) THEN it is skipped AND not
included in the discovered plugin list

#### Scenario: Windows executable detection

WHEN discovery runs on Windows THEN it looks for both `twig-<name>` and `twig-<name>.exe` as candidates AND strips the
`.exe` suffix from the plugin name for deduplication (e.g., `twig-prune.exe` becomes `prune`)

### Requirement: Collecting multiple plugin locations

#### Scenario: Plugin exists in multiple PATH directories

WHEN the same plugin name is found in multiple directories on PATH THEN all locations are collected in PATH order AND
the first location is designated the primary path AND the primary path is used for file size computation AND all paths
are reported in the plugin listing

#### Scenario: Duplicate paths for the same plugin are excluded

WHEN the exact same path appears multiple times for a plugin THEN only one entry is kept

### Requirement: Plugin metadata

#### Scenario: PluginInfo fields

WHEN a plugin is discovered THEN its `PluginInfo` contains: `name` (without the `twig-` prefix), `paths` (all locations
in PATH order), and `size_in_bytes` (file size of the primary path)

### Requirement: Canonical path resolution

#### Scenario: Resolving symlinks

WHEN a plugin path is found THEN twig attempts to canonicalize it (resolve symlinks and relative path components) AND if
canonicalization fails, the original path is used as a fallback

## External Command Dispatch

### Requirement: Fallthrough from unrecognized subcommands

#### Scenario: Unknown subcommand matches an installed plugin

WHEN the user runs `twig <name>` AND `<name>` does not match any built-in subcommand AND a plugin named `twig-<name>` is
found on PATH THEN the plugin binary is executed with the remaining CLI arguments AND the plugin inherits stdin, stdout,
and stderr from the parent process

#### Scenario: Unknown subcommand with no matching plugin

WHEN the user runs `twig <name>` AND `<name>` does not match any built-in subcommand AND no plugin named `twig-<name>`
is found on PATH THEN the command prints an error: "'<name>' is not a twig command or installed plugin" AND prints a tip:
"External plugins are executables named 'twig-<name>' on your PATH. Run 'twig self plugins' to list installed plugins."

### Requirement: Plugin exit code propagation

#### Scenario: Plugin exits with a status code

WHEN a plugin process exits THEN the parent twig process exits with the same exit code AND if the plugin's exit code
cannot be determined, twig exits with code 1

### Requirement: Plugin execution logging

#### Scenario: Verbose mode logging

WHEN a plugin is executed with verbose mode enabled THEN twig logs the plugin binary name and path at debug level AND
logs the plugin's exit status at debug level

## Environment Variables

### Requirement: Context variables set for plugin execution

#### Scenario: Variables always set

WHEN twig executes a plugin THEN it sets the following environment variables: `TWIG_CONFIG_DIR` (path to twig config
directory), `TWIG_DATA_DIR` (path to twig data directory), `TWIG_VERSION` (semantic version of the running twig binary
from `CARGO_PKG_VERSION`), `TWIG_COLORS` (color mode: `"yes"`, `"no"`, or `"auto"` reflecting the `--colors` flag),
`TWIG_NO_LINKS` (terminal hyperlink preference: `"0"` to enable or `"1"` to disable, reflecting the `--no-links` flag),
and `TWIG_VERBOSITY` (verbosity level 0–3 reflecting the `-v` flag count)

#### Scenario: Variables conditionally set

WHEN twig executes a plugin AND a git repository is detected in the current directory THEN `TWIG_CURRENT_REPO` is set to
the absolute path of the repository AND `TWIG_CURRENT_BRANCH` is set to the current branch name

WHEN twig executes a plugin AND no git repository is detected THEN `TWIG_CURRENT_REPO` and `TWIG_CURRENT_BRANCH` are not
set

## Plugin Context Discovery (`twig-core`)

### Requirement: PluginContext::discover()

#### Scenario: Environment variable priority

WHEN a plugin calls `PluginContext::discover()` THEN environment variables are preferred over auto-detection AND
`TWIG_CONFIG_DIR` is used for the config directory (falling back to `get_config_dirs()` if unset) AND `TWIG_DATA_DIR` is
used for the data directory (falling back to `get_config_dirs()` if unset) AND `TWIG_CURRENT_REPO` is used for the
repository path (falling back to `detect_repository()` if unset) AND `TWIG_CURRENT_BRANCH` is used for the branch name
(falling back to reading the repository HEAD if unset and a repository is available)

#### Scenario: Color mode parsing

WHEN `TWIG_COLORS` is read by `PluginContext::discover()` THEN `"yes"` (case-insensitive) maps to `ColorMode::Yes` AND
`"no"` (case-insensitive) maps to `ColorMode::No` AND any other value maps to `ColorMode::Auto`

#### Scenario: No-links flag parsing

WHEN `TWIG_NO_LINKS` is read by `PluginContext::discover()` THEN `"1"` maps to `true` (links disabled) AND any other
value maps to `false` (links enabled)

#### Scenario: Verbosity parsing

WHEN `TWIG_VERBOSITY` is read by `PluginContext::discover()` THEN the value is parsed as a `u8` AND if parsing fails,
it defaults to `0`

### Requirement: Plugin-specific directories

#### Scenario: Computing plugin config and data paths

WHEN a plugin calls `plugin_config_dir(name)` THEN the path `{config_dir}/plugins/{name}` is returned AND when a plugin
calls `plugin_data_dir(name)` THEN the path `{data_dir}/plugins/{name}` is returned AND these directories are not
auto-created

## Plugin Listing Command

### Requirement: Displaying discovered plugins

#### Scenario: Plugins are found

WHEN the user runs `twig self plugins` (alias `twig self list-plugins`) AND one or more plugins are discovered THEN the
command prints "Available Twig plugins" AND for each plugin prints the full binary name (e.g., `twig-flow`), the primary
path, and the file size formatted in human-readable units (B, KiB, MiB, GiB, TiB) AND if the plugin exists in multiple
PATH locations, lists alternate paths under an "Also found at:" subheader

#### Scenario: No plugins are found

WHEN the user runs `twig self plugins` AND no plugins are discovered THEN the command prints a warning "No Twig plugins
were found in your PATH." AND prints guidance "Add executables named `twig-<command>` to a directory on your PATH to
enable plugins." AND the command returns successfully (does not fail)

## Library Usage

### Requirement: Plugins as twig-core consumers

#### Scenario: Plugin depends on twig-core as a library

WHEN a plugin is implemented as a Rust binary THEN it can add `twig-core`, `twig-gh`, or `twig-jira` as Cargo
dependencies AND use their public APIs directly (e.g., `RepoState`, `GitHubRepo`, `detect_repository()`,
`delete_local_branch()`) AND use twig-core output functions (`print_info`, `print_success`, `print_error`,
`print_warning`) for consistent user-facing messages
