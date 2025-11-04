# Twig Agent Guide

## Project Overview

- Workspace of Rust crates; `twig-cli` (entrypoint), `twig-core` (shared state/config/output), `twig-jira` and `twig-gh` (service clients), plus `twig-test-utils` for integration helpers. 【F:Cargo.toml†L1-L60】
- `twig` binary crate re-exports `twig-cli`; most logic lives under `twig-cli/src` with clap-driven command modules mapped one-to-one to subcommands (e.g. `cli/git.rs`, `cli/jira.rs`). 【F:twig-cli/src/main.rs†L5-L46】【F:twig-cli/src/cli/mod.rs†L1-L108】
- Shared persistent state lives in JSON/TOML under XDG dirs managed by `twig-core::config`; command handlers manipulate registries/worktrees via `twig-core::state`. 【F:twig-core/src/config.rs†L1-L96】【F:twig-core/src/state.rs†L1-L93】

## Documentation Workflow

- Long-form specifications and agent-readable plans live under `docs/specs/`. Review the [directory README](docs/specs/README.md) for naming conventions (`YYYY-MM-DD_descriptive-name.md`), the `_TEMPLATE.md` starter, and guidance on incremental checkpointing/lessons-learned when collaborating via docs. Ensure new specs follow this workflow to keep AI/human collaboration in sync. 【F:docs/specs/README.md†L1-L104】

## Build & Test Workflow

- Prefer Make targets: `make fmt` (runs `cargo fmt`, `cargo clippy --fix`, Ruff for plugin examples), `make lint`, `make test` (`cargo nextest run --workspace`). 【F:Makefile†L1-L58】
- When building a single crate outside Make, run `cargo build -p <crate-name>` to stay within the workspace context. 【F:Cargo.toml†L1-L60】
- CI expectations: never call `cargo test`; default to `cargo nextest run` when Make is unavailable. 【F:Makefile†L39-L42】
- Use `make check` for quick type-checking via `cargo check --workspace`. 【F:Makefile†L31-L34】
- Coverage via `make coverage` (wraps `cargo llvm-cov nextest`). Release binaries built with `make release` (workspace build).

## Command Architecture

- Each command module defines clap structs and a `handle_*` function that delegates to helper modules (see `cli/git.rs` calling `crate::git::...`). Maintain this separation: parsing stays in `cli/`, IO/workflows in sibling modules. 【F:twig-cli/src/cli/git.rs†L1-L91】
- The top-level `handle_cli` in `cli/mod.rs` orchestrates plugin routing and subcommand dispatch; follow existing patterns for new subcommands. 【F:twig-cli/src/cli/mod.rs†L1-L160】
- `twig-cli/src/clients.rs` centralizes Jira/GitHub client instantiation, including synchronous Tokio runtime creation—reuse helpers instead of constructing clients ad hoc. 【F:twig-cli/src/clients.rs†L1-L80】

## State & Persistence

- Global registry stored at `${XDG_DATA_HOME}/twig/registry.json`; use `twig_core::state::Registry` helpers instead of manual file IO. 【F:twig-core/src/config.rs†L49-L76】【F:twig-core/src/state.rs†L24-L83】
- Repo-local metadata lives in `.twig/state.json` per repo (`ConfigDirs::repo_state_path`). Respect this layout when adding new persisted fields. 【F:twig-core/src/config.rs†L64-L92】
- Jira parsing behaviour configurable via `${XDG_CONFIG_HOME}/twig/jira.toml`; leverage `ConfigDirs::load_jira_config()` and `save_jira_config()` to remain compatible. 【F:twig-core/src/config.rs†L92-L140】

## Output & Tracing Conventions

- User-facing messages go through `twig_core::output::{print_success, print_error, print_warning, print_info}`; verbose diagnostics should use `tracing` macros to honor `-v/-vv/-vvv`. 【F:twig-core/src/output.rs†L1-L80】
- CLI initializes tracing level from `-v` flag; avoid custom logging setups. 【F:twig-cli/src/main.rs†L24-L44】

## Integrations

- Service credentials read from `.netrc`; reuse `creds` helpers when accessing GitHub/Jira. `create_*_runtime_and_client` returns a ready-to-use Tokio runtime + client for async workflows. 【F:twig-cli/src/clients.rs†L21-L70】
- HTTP clients live in `twig-jira` and `twig-gh` crates with `endpoints/` + `models.rs`. Extend APIs there, then expose via `twig-cli::clients`.

## Testing & Fixtures

- Snapshot-heavy tests use `insta`; update snapshots with `make update-snapshots` then review via `make insta-review`. 【F:Makefile†L34-L56】
- Shared test utilities (temp repos, config dirs, netrc handling) live in `twig-test-utils`. Import helpers instead of reimplementing scaffolding. 【F:twig-test-utils/src/lib.rs†L1-L60】

## Generated & External Files

- Example plugin code under `examples/plugins/` has Ruff formatting hooks; run `make fmt` to keep Python lint happy when touching these examples. 【F:Makefile†L12-L24】

## Extensibility Notes

- Plugin lifecycle: when CLI arguments do not match a subcommand, control falls through to plugin execution defined in `plugin.rs`. Follow existing pattern for new plugin discovery or metadata changes.
- Auto-dependency features combine static config (`state.rs`) with heuristics in `auto_dependency_discovery.rs`; consult both before altering branch-tree logic.

---

_Questions or unclear sections? Ask for feedback so we can refine this guide._
