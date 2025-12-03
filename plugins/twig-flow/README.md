# twig-flow Plugin

Rust-based Twig plugin for branch tree visualization and switching, modeled after argit but aligned with Twig conventions.

## Installation

- Build and install from the workspace: `cargo install --path plugins/twig-flow`.
- Ensure the resulting `twig-flow` binary is on your `PATH` so the Twig CLI can discover it via plugin lookup.

## Usage

- `twig flow`: render the local branch tree with table columns (Branch/Story/PR/Notes) and highlight the current branch.
- `twig flow --root`: checkout the configured root branch (e.g., `main`) before rendering.
- `twig flow --parent`: checkout the current branch's primary parent before rendering (errors if multiple parents exist).
- `twig flow <target>`: switch to a branch (existing or new) using the shared switch engine; Jira keys and PR references are resolved with Twig helpers.

## Behavior & Architecture

- Parsing and dispatch live in this plugin crate; shared graph building, rendering, and switching logic come from `twig-core`.
- Output uses `twig_core::output` helpers for consistent styling and honors color/no-color settings.
- Tree rendering uses the `BranchTableRenderer` from `twig-core`, keeping column alignment stable for snapshot tests and CLI output.

## Configuration

- Respects standard Twig config/state locations via `twig_core::config::ConfigDirs`.
- Column schema overrides and other advanced tweaks are intentionally hidden for now; future specs will cover exposing them.

## Troubleshooting

- Not in a git repo: the plugin emits an actionable error; initialize or `cd` into a repository.
- Multiple parents with `--parent`: currently errors and lists options; interactive selection is deferred to a future iteration.
- Detached HEAD or empty repo: renderer falls back to header-only output with warnings.

## Contributing & Testing

- Format/lint/test via workspace defaults: `make fmt`, `make test` (uses `cargo nextest`), `make lint` as needed.
- Integration tests live under `plugins/twig-flow/tests` and use `twig-test-utils` for repo fixtures.
