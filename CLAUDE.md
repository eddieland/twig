# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Twig is a Git-first productivity tool written in Rust that manages branches, issues, and pull requests across one or
many repositories. Core focus: stacked pull-request workflows with cascading rebases.

## Build & Development

**Requirements**: Rust 1.91.0+, nightly toolchain (auto-detected from `rust-toolchain.toml`)

```bash
# Prefer Make targets for common tasks
make build              # Debug build (workspace + plugins)
make release            # Release build with LTO
make check              # Quick type-check via cargo check
make fmt                # Format + clippy --fix + Ruff for Python examples
make lint               # cargo clippy --workspace -- -D warnings
```

For single crate operations within the workspace: `cargo build -p <crate-name>`

## Testing

**CRITICAL**: Always use nextest, never `cargo test`

```bash
make test               # Run all tests (workspace + plugins)
cargo nextest run       # Direct nextest invocation
cargo nextest run -p twig-core  # Single crate
cargo nextest run -E 'test(my_test_name)'  # Single test by name
```

**Exception**: Doc tests require `cargo test --doc` (nextest doesn't support them):

```bash
cargo test -p twig-core --doc   # Run doc tests for a crate
```

**Snapshot testing with Insta**:

```bash
make insta-review       # Interactive review
make insta-accept       # Accept all pending
make update-snapshots   # Run tests with INSTA_UPDATE=1
```

## Workspace Architecture

```
twig/
├── twig-cli/          # Main CLI binary, clap command modules
├── twig-core/         # Shared: config, state, git ops, output
├── twig-gh/           # GitHub API client + endpoints
├── twig-jira/         # Jira API client + endpoints
├── no-worries/        # Utility library
├── twig-test-utils/   # Test fixtures: temp repos, config dirs, netrc
└── plugins/twig-flow/ # Example Rust plugin
```

## Command Architecture

- Each command module in `twig-cli/src/cli/` defines clap structs + `handle_*` function
- Parsing stays in `cli/`, business logic in sibling modules
- `twig-cli/src/clients.rs` centralizes Jira/GitHub client instantiation with Tokio runtime

## State & Persistence

- Global registry: `${XDG_DATA_HOME}/twig/registry.json` — use `twig_core::state::Registry`
- Per-repo metadata: `.twig/state.json` — use `ConfigDirs::repo_state_path`
- Jira config: `${XDG_CONFIG_HOME}/twig/jira.toml` — use `ConfigDirs::load_jira_config()`
- Credentials: `~/.netrc` — use `creds` helpers

## Output & Logging

- User messages: `twig_core::output::{print_success, print_error, print_warning, print_info}`
- Verbose diagnostics: `tracing` macros (honored by `-v/-vv/-vvv`)

## Code Quality

Pre-commit hooks enforce formatting and linting. Setup: `make pre-commit-setup`

Workspace-level clippy lints prohibit: `unwrap_used`, `panic`, `print_stdout`, `print_stderr`, `dbg_macro`, `todo`,
`unimplemented`

## Test Utilities

`twig-test-utils` provides RAII guards for isolated testing:

- `GitRepoTestGuard::new_and_change_dir()` — temp git repo with auto-cleanup
- `EnvTestGuard` / `HomeEnvTestGuard` — XDG/HOME overrides
- `setup_test_env_with_registry()` — complete test environment

## Documentation Workflow

Specs live in `docs/specs/` with naming convention `YYYY-MM-DD_descriptive-name.md`. Use `_TEMPLATE.md` for new specs.
Practice incremental checkpointing for long-running work.

## Plugin System

Plugins named `twig-<name>` discovered via `$PATH`. Receive context via env vars: `TWIG_CONFIG_DIR`, `TWIG_DATA_DIR`,
`TWIG_CURRENT_REPO`, `TWIG_CURRENT_BRANCH`, `TWIG_VERSION`
